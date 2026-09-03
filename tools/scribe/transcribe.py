"""Turn a video into a transcript, locally.

    python tools/scribe/transcribe.py <youtube-url|path-to-file> [--model medium]

Writes two files next to each other in tools/scribe/transcripts/:
a .txt of the spoken words, and a .json carrying the segments with their
timestamps, because a written piece sometimes needs to know where in the video
something was said and that information cannot be recovered later.

Why local rather than an API: the machine already has ffmpeg and faster-whisper,
the audio is Richard's own voice, and a pipeline that sends it to a third party
to be typed would contradict the product it is going to be writing about. The
cost is time on a CPU, which is real and is stated by the tool rather than
hidden: it prints the audio duration before it starts.

Two ways in, and the choice is a real trade rather than a formality:

  --captions  reads the track YouTube already generated. Instant, even on a long
              video, which is why the online tools that feel magic are doing
              exactly this. Measured cost: YouTube's automatic Portuguese arrives
              with no punctuation and no capitals, so sentence boundaries have to
              be decided downstream.

  --asr       listens to the audio with faster-whisper. Slower, roughly the
              length of the video on this CPU, and it returns punctuated,
              capitalised text with better word accuracy.

The --asr path is handed the project's own vocabulary, because a decoder that has
never seen the word Ulpia writes Upia, and it wrote Upia in all five places the
name was spoken on the launch video. The list is VOCABULARY below, and adding to
it is part of correcting a mishearing rather than a separate chore.

Default: captions when they exist, audio when they do not, because most of the
time the words are the expensive part and the punctuation is not. Pass --asr when
the recording matters enough to wait.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import unicodedata
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT_DIR = HERE / "transcripts"

# The decoder's vocabulary, and every line of it is a measurement rather than a
# guess. On the launch video, 8.7 minutes through large-v3, whisper produced
# "Upia" in all five places Ulpia was spoken, "Memo", "Zap" and "Letra" for Mem0,
# Zep and Letta, "DeepSea" for DeepSeek, "OpenStack Tem Source" for open source,
# and "chipar" for shipar. Every one was corrected by hand afterwards, and would
# be corrected by hand again on the next recording. That is the work this removes.
#
# It is hardcoded here rather than kept in a file beside the script or passed on a
# flag. A flag is the worst of the three: it has to be remembered on every run, and
# a vocabulary nobody remembers to pass is a vocabulary that does not exist. A
# separate file rots exactly as fast as this list does and adds a second place to
# look, since the only people who edit it are already editing this file.
#
# The rule that keeps it current: when you correct a misheard technical term by
# hand, add it here in the same move. A term corrected twice is a term that should
# have been added the first time.
TERMS = [
    "Ulpia", "Vesta",
    "Mem0", "Zep", "Letta", "Cognee",
    "DeepSeek", "Anthropic", "OpenAI",
    "MCP", "SQLite", "BM25", "embedding",
]

# Terms whose spelling belongs to one language. They are kept apart from the names
# above because a name is the same word everywhere and these are not: "open source"
# only needs help when it is spoken inside Portuguese, and "shipar" is Brazilian
# developer slang that would be noise pushed at an English recording.
TERMS_BY_LANGUAGE = {
    "pt": ["open source", "shipar"],
}

# The shape of the hint is not cosmetic, and this is the part that was measured
# rather than assumed. Whisper reads this block as if it were speech that came just
# before the audio, so a bare comma list conditions weakly: on a 44 second clip of
# the launch video it recovered Zep and Letta and still wrote "Upia" and "Memo".
# The same terms wrapped in a sentence that puts the name in the grammatical frame
# the audio uses recovered all four. So the hint is a carrier sentence, and a
# carrier sentence has a language, which is why there is one per language rather
# than one for both.
#
# With --language omitted there is no language to write a sentence in, because the
# hint has to be built before the call that detects it. That case falls back to the
# bare list, which is weaker, and passing --language is worth it for more than this.
CARRIER = {
    "pt": ("Neste vídeo eu falo sobre o projeto Ulpia. O Ulpia é uma memória local "
           "para agentes de IA. Também aparecem: {terms}."),
    "en": ("In this video I talk about the Ulpia project. Ulpia is a local memory "
           "for AI agents. Also mentioned: {terms}."),
}

# faster-whisper truncates the hint at max_length // 2 - 1, which is 223 tokens
# (faster_whisper/transcribe.py, get_prompt), and it truncates from the end,
# silently, so growth costs you the newest entries first. Latin script runs roughly
# four characters to the token, so this is a conservative line at which to say
# something out loud rather than let the tail disappear.
VOCABULARY_CHAR_BUDGET = 600


def vocabulary_for(language: str | None) -> str:
    """The hint handed to the decoder: a carrier sentence, or a bare list."""
    terms = ", ".join(TERMS + TERMS_BY_LANGUAGE.get(language or "", []))
    hint = CARRIER[language].format(terms=terms) if language in CARRIER else terms
    if len(hint) > VOCABULARY_CHAR_BUDGET:
        print(f"scribe: the vocabulary is {len(hint)} characters and the model keeps "
              f"about {VOCABULARY_CHAR_BUDGET}. It drops the end of the list without "
              f"saying so, so shorten it rather than trusting the tail.")
    return hint


def run(cmd, **kw):
    """Run a command and fail loudly, because a silent failure here produces an
    empty transcript that looks like a short video."""
    proc = subprocess.run(cmd, capture_output=True, text=True, **kw)
    if proc.returncode != 0:
        sys.exit(f"scribe: {cmd[0]} failed\n{proc.stderr[-2000:]}")
    return proc.stdout


def is_url(source: str) -> bool:
    return source.startswith("http://") or source.startswith("https://")


def fetch_captions(source: str, workdir: Path, lang: str | None) -> tuple[str, list, str] | None:
    """Return (text, segments, language) from YouTube's own track, or None.

    Preference order matters: a track the uploader wrote is punctuated and
    correct, and is worth more than anything a machine produces. The automatic
    one is the fallback within the fallback.
    """
    listing = run(["yt-dlp", "--no-warnings", "--list-subs", source])
    wanted = []
    if lang:
        wanted = [lang, f"{lang}-orig"]
    else:
        # -orig is the language actually spoken; the others are translations of
        # it, and translating our own words back to us would be absurd.
        for code in re.findall(r"^([a-z]{2}(?:-[A-Za-z]+)?)\s", listing, re.M):
            if code.endswith("-orig"):
                wanted.insert(0, code)
            elif code not in wanted:
                wanted.append(code)
    if not wanted:
        return None

    for code in wanted[:3]:
        out = workdir / "subs"
        proc = subprocess.run(
            ["yt-dlp", "--no-warnings", "--skip-download", "--write-subs",
             "--write-auto-subs", "--sub-langs", code, "--sub-format", "srt",
             "-o", str(out), source],
            capture_output=True, text=True,
        )
        hits = sorted(workdir.glob("subs*.srt"))
        if proc.returncode == 0 and hits:
            return (*parse_srt(hits[0].read_text(encoding="utf-8")), code)
    return None


BLANK_LINE = re.compile(chr(10) + r"\s*" + chr(10))


def parse_srt(raw: str) -> tuple[str, list]:
    """SRT to plain text plus segments. yt-dlp already collapses the rolling
    window that auto-captions use, so consecutive cues do not repeat."""
    segments = []
    for block in re.split(BLANK_LINE, raw.strip()):
        lines = [l for l in block.splitlines() if l.strip()]
        if len(lines) < 2:
            continue
        stamp = next((l for l in lines if "-->" in l), None)
        if not stamp:
            continue
        body = " ".join(lines[lines.index(stamp) + 1:]).strip()
        if not body:
            continue

        def secs(t):
            h, m, rest = t.split(":")
            s, ms = rest.replace(".", ",").split(",")
            return int(h) * 3600 + int(m) * 60 + int(s) + int(ms) / 1000

        a, b = [p.strip() for p in stamp.split("-->")]
        segments.append({"start": round(secs(a), 2), "end": round(secs(b), 2), "text": body})
    return " ".join(s["text"] for s in segments), segments


def slugify(text: str) -> str:
    # Accents are stripped rather than kept: the slug becomes a URL, and an
    # address with combining marks is legal, ugly, and a trap for anyone who
    # types it by hand or pastes it into a terminal.
    text = unicodedata.normalize("NFKD", text)
    text = "".join(c for c in text if not unicodedata.combining(c))
    text = re.sub(r"[^A-Za-z0-9\s-]", "", text).strip().lower()
    return re.sub(r"[\s_-]+", "-", text)[:70] or "untitled"


def fetch_audio(source: str, workdir: Path) -> tuple[Path, str]:
    """Return a wav path and a human title. yt-dlp for links, ffmpeg for files."""
    if is_url(source):
        if not shutil.which("yt-dlp"):
            sys.exit("scribe: yt-dlp is not installed, and the source is a link")
        title = run(["yt-dlp", "--no-warnings", "--print", "title", source]).strip()
        # Ask for audio only: downloading video to throw the picture away costs
        # bandwidth and time for nothing.
        run([
            "yt-dlp", "--no-warnings", "-f", "bestaudio", "-x",
            "--audio-format", "wav", "-o", str(workdir / "audio.%(ext)s"), source,
        ])
    else:
        path = Path(source).expanduser()
        if not path.exists():
            sys.exit(f"scribe: no such file: {path}")
        title = path.stem
        # 16kHz mono is what the model listens to; anything richer is discarded
        # inside the model anyway and only makes the file bigger.
        run([
            "ffmpeg", "-nostdin", "-y", "-i", str(path),
            "-vn", "-ac", "1", "-ar", "16000", str(workdir / "audio.wav"),
        ])
    wav = workdir / "audio.wav"
    if not wav.exists():
        sys.exit("scribe: no audio was produced")
    return wav, title


def main() -> None:
    ap = argparse.ArgumentParser(description="Transcribe a video into text, locally.")
    ap.add_argument("source", help="a YouTube link, or a path to a local video or audio file")
    ap.add_argument("--model", default="medium",
                    help="faster-whisper model: tiny, base, small, medium, large-v3. "
                         "medium is the default because large-v3 on a CPU turns a long "
                         "video into an afternoon")
    ap.add_argument("--language", default=None,
                    help="force a language code such as pt or en; detected when omitted")
    # Additive, and the default is the whole point: omitted, this writes exactly where
    # it always has, so the scribe's own workflow is unchanged. Given, it drops the
    # transcript straight into another base's inbox/, which is what a recording fetched
    # FOR somebody else needs. Raw material has to land where its reader already looks,
    # or it lands in a shared folder that only the person who put it there knows about.
    ap.add_argument("--out", default=None, metavar="DIR",
                    help="write the .txt and .json here instead of tools/scribe/transcripts/")
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument("--captions", action="store_true",
                      help="use YouTube's own caption track and fail if there is none")
    mode.add_argument("--asr", action="store_true",
                      help="always listen to the audio, even when captions exist")
    args = ap.parse_args()

    out_dir = Path(args.out).expanduser() if args.out else OUT_DIR
    out_dir.mkdir(parents=True, exist_ok=True)

    # The fast path first, unless told otherwise.
    if is_url(args.source) and not args.asr:
        import tempfile as _t
        with _t.TemporaryDirectory() as tmp:
            title = run(["yt-dlp", "--no-warnings", "--print", "title", args.source]).strip()
            got = fetch_captions(args.source, Path(tmp), args.language)
        if got:
            text, segments, code = got
            stem = slugify(title)
            (out_dir / f"{stem}.txt").write_text(text, encoding="utf-8")
            (out_dir / f"{stem}.json").write_text(
                json.dumps({"source": args.source, "title": title, "language": code,
                            "method": "youtube-captions", "segments": segments},
                           ensure_ascii=False, indent=1), encoding="utf-8")
            print(f"scribe: took YouTube's own {code} track, {len(text.split())} words, instantly.")
            if text[:400].count(".") + text[:400].count("?") < 2:
                print("scribe: it arrived without punctuation, which is normal for an "
                      "automatic track. Run again with --asr if that matters here.")
            print(f"scribe: wrote {stem}.txt and {stem}.json in {out_dir}")
            return
        if args.captions:
            sys.exit("scribe: no caption track on that video")
        print("scribe: no captions on that video, listening to the audio instead")
    elif args.captions:
        sys.exit("scribe: --captions only applies to a link")

    try:
        from faster_whisper import WhisperModel
    except ImportError:
        sys.exit("scribe: faster-whisper is not installed (pip install faster-whisper)")

    with tempfile.TemporaryDirectory() as tmp:
        workdir = Path(tmp)
        print(f"scribe: fetching audio from {'the link' if is_url(args.source) else 'the file'}")
        wav, title = fetch_audio(args.source, workdir)

        print(f"scribe: loading {args.model} (the first run downloads it)")
        # int8 on CPU: the accuracy cost is small and the speed difference is
        # what makes a long video finish at all on this machine.
        model = WhisperModel(args.model, device="cpu", compute_type="int8")

        # hotwords rather than initial_prompt, and the difference is the whole
        # reason this works past the first minute. initial_prompt is pushed once
        # onto the front of the running token list, and every 30 second window is
        # conditioned on only the last 223 tokens of that list, so the vocabulary
        # is shoved off the end as soon as the transcript so far is longer than
        # about a minute of speech. It is also discarded outright whenever the
        # decoder falls back to a temperature above 0.5. hotwords is re-encoded
        # into every window's prompt instead, outside that list, so it survives
        # both. Read in faster_whisper/transcribe.py, get_prompt and the seek loop,
        # version 1.2.1. condition_on_previous_text stays at its default True and
        # does not interact with this: it only governs the running list.
        vocabulary = vocabulary_for(args.language)
        print(f"scribe: conditioning the decoder on {vocabulary.count(',') + 1} known terms")
        segments, info = model.transcribe(
            str(wav),
            language=args.language,
            vad_filter=True,  # drops silence, which is most of the pauses in speech
            beam_size=5,
            hotwords=vocabulary,
        )
        minutes = info.duration / 60
        print(f"scribe: {minutes:.1f} minutes of audio, language {info.language}. Working.")

        collected = []
        for seg in segments:
            collected.append({"start": round(seg.start, 2), "end": round(seg.end, 2),
                              "text": seg.text.strip()})
            # Progress on a long file, because a silent process for forty minutes
            # is indistinguishable from a hung one.
            if len(collected) % 25 == 0:
                print(f"  {collected[-1]['end'] / 60:5.1f} / {minutes:.1f} min", flush=True)

    stem = slugify(title)
    text = " ".join(s["text"] for s in collected)
    (out_dir / f"{stem}.txt").write_text(text, encoding="utf-8")
    (out_dir / f"{stem}.json").write_text(
        # The vocabulary goes in the sidecar for the same reason "method" does:
        # a name in this transcript was partly produced by the hint, and anyone
        # auditing a name later should be able to see which hint was in force.
        json.dumps({"source": args.source, "title": title, "language": info.language,
                    "method": f"whisper-{args.model}", "vocabulary": vocabulary,
                    "duration_s": round(info.duration, 1), "segments": collected},
                   ensure_ascii=False, indent=1),
        encoding="utf-8",
    )
    print(f"scribe: wrote {stem}.txt and {stem}.json in {out_dir} "
          f"({len(text.split())} words)")


if __name__ == "__main__":
    main()
