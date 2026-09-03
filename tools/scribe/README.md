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

**The decoder is handed the project's own vocabulary.** A model that has never
seen the word Ulpia writes Upia, and on the launch video it wrote Upia in all five
places the name was spoken, plus `Memo`, `Zap` and `Letra` for Mem0, Zep and Letta.
Those were corrected by hand afterwards, on a video that will have a successor next
week. The list is `TERMS` in `transcribe.py`, and adding to it is part of
correcting a mishearing rather than a separate chore.

Two mechanics of it are worth knowing before editing it, because both were
measured rather than assumed:

- **It goes in as `hotwords`, not `initial_prompt`.** `initial_prompt` is pushed
  once onto the front of a running token list, and each 30 second window sees only
  the last 223 tokens of that list, so the vocabulary falls off the end after
  roughly a minute of speech and is dropped outright on a temperature fallback.
  `hotwords` is re-encoded into every window. On a 44 second clip the two are
  identical; on an 8.7 minute video only one of them is still working at the end,
  and a hint that fixes the opening and quietly stops is worse than none, because
  it looks fixed.
- **It is a sentence, not a list.** Whisper reads the hint as speech that came just
  before the audio. A bare comma list of the same terms recovered Zep and Letta and
  still wrote `Upia` and `Memo`; the terms wrapped in a sentence that puts the name
  in the grammatical frame the audio uses recovered all four. A sentence has a
  language, which is why there is one per language and why `--language` is worth
  passing. Without it the hint falls back to the bare list.

**`publish.py`** takes the adapted markdown, writes it into the site's content
folder with the frontmatter the build requires, commits that one file through
`kb commit`, and pushes, which is what triggers the deploy.

```bash
python tools/scribe/publish.py piece.md --title "..." --description "..." --source "https://youtu.be/..."
python tools/scribe/publish.py piece.md --title "..." --lang pt-BR --dry-run
```

The date is written once and used twice, in the filename and in the frontmatter,
so the two cannot drift.

## The language, which is typed once by the caller

The site reads `lang` out of the frontmatter, and a Portuguese post served under
`lang="en"` mishyphenates and is read aloud wrong. So the field matters, and there
is exactly one way to set it.

- **`--lang <tag>`** writes it. A tag that cannot be parsed stops the run, because
  it was typed by a person and a typo should not become a silent default.
- **Omitted:** the field is left out entirely and `build-posts.mjs` applies its own
  default, which is the single place that default is written down. Nothing here
  guesses a language.

**`--transcript` used to exist and was removed, which is worth writing down so it
does not get reinvented.** It read the `language` the transcript's `.json` sidecar
carried and wrote that as `lang`. The mechanism worked; the premise did not. The
sidecar records the language of the **recording**, and this field is the language
of the **post**. Those two agreed only for as long as a Portuguese recording became
a Portuguese post, and a post is now written in one fixed language whatever was
spoken. So the recording's language is never the answer to the frontmatter's
question, and a flag that supplies the wrong answer is a trap rather than a
convenience. It was deleted rather than documented as deprecated for that reason.

**Casing and `-orig`.** `normalise_lang` lowercases the language subtag, uppercases
a two letter region so `pt-br` is written `pt-BR`, and strips yt-dlp's `-orig`
marker, which marks the track in the language actually spoken and is not a BCP 47
subtag. There is deliberately no table mapping `pt` to `pt-BR`: such a table needs
editing for every new language and is wrong for the one post that wanted the other
tag. Want the region? Pass it.

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
