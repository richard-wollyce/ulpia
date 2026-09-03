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
python tools/scribe/publish.py piece.md --title "..." --transcript tools/scribe/transcripts/piece.json
python tools/scribe/publish.py piece.md --title "..." --lang pt-BR --dry-run
```

The date is written once and used twice, in the filename and in the frontmatter,
so the two cannot drift.

## The language, which is detected once and then carried

`transcribe.py` already knows the language: it is in the `.json` sidecar as
`language`, on both the caption path and the whisper path. The site reads `lang`
out of the frontmatter, and a Portuguese post served under `lang="en"`
mishyphenates and is read aloud wrong. So the sidecar is where the value comes
from, rather than somebody's memory at publish time.

- **`--transcript <the .json>`** reads that field and writes it as `lang`. The
  path is named by the caller and nothing searches for it, because there is no
  convention tying an adapted piece back to its transcript: `transcribe.py --out`
  drops transcripts into other bases, and a folder search picks the wrong file in
  silence exactly when two recordings are in flight at once. A `.txt` or a bare
  stem is accepted and the sidecar beside it is read.
- **`--lang <tag>`** sets it directly and wins over the sidecar. A tag that
  cannot be parsed stops the run, because it was typed by a person.
- **Neither, or a sidecar with no language:** the field is left out entirely and
  the build keeps its `en` default. Nothing here guesses a language.

**`pt` and not `pt-BR`, unless you ask.** Whisper reports two letter codes; both
tags are valid BCP 47 and both hyphenate. What was detected is what gets written,
and there is deliberately no table mapping detected codes to preferred ones,
because such a table needs editing for every new language and is wrong for the
one post that wanted the other tag. Want the region? Pass `--lang pt-BR`. The
only rewriting done is stripping yt-dlp's `-orig` marker, which marks the track
in the language actually spoken and is not a BCP 47 subtag.

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
