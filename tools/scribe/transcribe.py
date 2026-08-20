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
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument("--captions", action="store_true",
                      help="use YouTube's own caption track and fail if there is none")
    mode.add_argument("--asr", action="store_true",
                      help="always listen to the audio, even when captions exist")
    args = ap.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)

    # The fast path first, unless told otherwise.
    if is_url(args.source) and not args.asr:
        import tempfile as _t
        with _t.TemporaryDirectory() as tmp:
            title = run(["yt-dlp", "--no-warnings", "--print", "title", args.source]).strip()
            got = fetch_captions(args.source, Path(tmp), args.language)
        if got:
            text, segments, code = got
            stem = slugify(title)
            (OUT_DIR / f"{stem}.txt").write_text(text, encoding="utf-8")
            (OUT_DIR / f"{stem}.json").write_text(
                json.dumps({"source": args.source, "title": title, "language": code,
                            "method": "youtube-captions", "segments": segments},
                           ensure_ascii=False, indent=1), encoding="utf-8")
            print(f"scribe: took YouTube's own {code} track, {len(text.split())} words, instantly.")
            if text[:400].count(".") + text[:400].count("?") < 2:
                print("scribe: it arrived without punctuation, which is normal for an "
                      "automatic track. Run again with --asr if that matters here.")
            print(f"scribe: wrote transcripts/{stem}.txt and transcripts/{stem}.json")
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

        segments, info = model.transcribe(
            str(wav),
            language=args.language,
            vad_filter=True,  # drops silence, which is most of the pauses in speech
            beam_size=5,
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
    (OUT_DIR / f"{stem}.txt").write_text(text, encoding="utf-8")
    (OUT_DIR / f"{stem}.json").write_text(
        json.dumps({"source": args.source, "title": title, "language": info.language,
                    "method": f"whisper-{args.model}",
                    "duration_s": round(info.duration, 1), "segments": collected},
                   ensure_ascii=False, indent=1),
        encoding="utf-8",
    )
    print(f"scribe: wrote transcripts/{stem}.txt and transcripts/{stem}.json "
          f"({len(text.split())} words)")


if __name__ == "__main__":
    main()
