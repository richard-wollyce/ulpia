# scribe

Two scripts that carry a recording to `ulpia.io/blog/`. The judgement in the
middle is not here, and that is deliberate.

```
video ──► transcribe.py ──► transcript ──► [the agent adapts] ──► publish.py ──► live
```

## What each half does

**`transcribe.py`** turns a YouTube link or a local file into text, on this
machine, with `faster-whisper`. It writes a `.txt` of the words and a `.json`
that keeps the timestamps, since a piece sometimes needs to point at a moment in
the video and that information cannot be recovered afterwards.

```bash
python tools/scribe/transcribe.py "https://youtube.com/watch?v=..."
python tools/scribe/transcribe.py "C:/path/to/video.mp4" --model large-v3
```

Models, and the trade: `medium` is the default because `large-v3` on a CPU turns
a long video into an afternoon. `large-v3` is noticeably better on Portuguese and
is worth the wait when the recording matters. The first run of any model
downloads it.

**`publish.py`** takes the adapted markdown, writes it into the site's content
folder with the frontmatter the build requires, commits that one file through
`kb commit`, and pushes, which is what triggers the deploy.

```bash
python tools/scribe/publish.py piece.md --title "..." --description "..." --source "https://youtu.be/..."
python tools/scribe/publish.py piece.md --title "..." --dry-run
```

The date is written once and used twice, in the filename and in the frontmatter,
so the two cannot drift.

## What is not automated, and why

The step between them. Turning a transcript into a piece is judgement about a
voice: which repetitions were thinking aloud and which were emphasis, where a
change of subject deserves a heading, which sentence was abandoned mid-way and
which was meant to trail off. A script that made those calls would be inventing
sentence boundaries, and inventing sentence boundaries is exactly where someone
else's voice enters a transcript.

That is the agent's work, and the standard it is held to is in its constitution
rather than here.

## Why local rather than an API

The machine already had `ffmpeg` and `faster-whisper`. The audio is Richard's
own voice, and a pipeline that shipped it to a third party to be typed would
contradict the product it is going to be writing about.
