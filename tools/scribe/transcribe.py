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

Why Whisper rather than YouTube's own captions, which are free and instant:
auto-captions arrive without punctuation, so the sentence boundaries have to be
invented downstream by whoever adapts the text. Inventing sentence boundaries is
exactly the step where someone else's voice creeps into a transcript, and this
pipeline exists to keep one voice intact.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
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


def slugify(text: str) -> str:
    text = re.sub(r"[^\w\s-]", "", text, flags=re.UNICODE).strip().lower()
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
    args = ap.parse_args()

    try:
        from faster_whisper import WhisperModel
    except ImportError:
        sys.exit("scribe: faster-whisper is not installed (pip install faster-whisper)")

    OUT_DIR.mkdir(parents=True, exist_ok=True)

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
                    "duration_s": round(info.duration, 1), "segments": collected},
                   ensure_ascii=False, indent=1),
        encoding="utf-8",
    )
    print(f"scribe: wrote transcripts/{stem}.txt and transcripts/{stem}.json "
          f"({len(text.split())} words)")


if __name__ == "__main__":
    main()
