"""Put an adapted piece into the site and publish it.

    python tools/scribe/publish.py post.md --title "..." --description "..." [--date YYYY-MM-DD]
    python tools/scribe/publish.py post.md --title "..." --lang pt-BR --dry-run

Takes a markdown body, writes it into the site's content folder with the
frontmatter the build requires, commits exactly that one file, and pushes, which
is what triggers the deploy.

The division of labour this file assumes: the adaptation from transcript to
written piece is the agent's work and cannot be automated here, because it is
judgement about a voice. What can be automated is everything after that
judgement, and everything after it is mechanical enough that doing it by hand
invites the two mistakes this file exists to prevent: a filename that disagrees
with the date in the frontmatter, and a commit that carries somebody else's work
because the paths were not named.

The language is typed, once, by the caller. It used to be read out of the
transcript's sidecar, and that was wrong for a reason worth keeping written down:
the sidecar carries the language of the recording, and this field is the language
of the post. Those agreed only for as long as a Portuguese recording became a
Portuguese post. They are two different questions, so the answer to one is never
the answer to the other, and a flag that supplies the wrong one is a trap rather
than a convenience.
"""

import argparse
import datetime
import re
import subprocess
import unicodedata
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
POSTS = REPO / "site" / "frontend" / "content" / "posts"


def slugify(text: str) -> str:
    # Accents are stripped rather than kept: the slug becomes a URL, and an
    # address with combining marks is legal, ugly, and a trap for anyone who
    # types it by hand or pastes it into a terminal.
    text = unicodedata.normalize("NFKD", text)
    text = "".join(c for c in text if not unicodedata.combining(c))
    text = re.sub(r"[^A-Za-z0-9\s-]", "", text).strip().lower()
    return re.sub(r"[\s_-]+", "-", text)[:70]


# Language tags as they arrive from detection are not always tags the web
# accepts. yt-dlp marks the track in the language actually spoken by appending
# "-orig" to the code, and "orig" is four letters, which BCP 47 reads as a
# script subtag that no registry has. Stripping that suffix is the only
# rewriting done here. There is deliberately no table mapping "pt" to "pt-BR":
# a preferred-tag table has to be edited every time a new language shows up and
# is wrong for the one post that wanted the other tag. The tag is whatever the
# caller passed, and "pt" and "pt-BR" are both valid BCP 47 and both hyphenate.
BCP47 = re.compile(r"^[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8})*$")


def normalise_lang(code: str) -> str | None:
    """A detected code as a tag fit for <html lang>, or None if it is not one."""
    code = (code or "").strip()
    if code.endswith("-orig"):
        code = code[: -len("-orig")]
    if not BCP47.match(code):
        return None
    parts = code.split("-")
    # Canonical casing, which is convention rather than requirement: language
    # lowercase, a two letter region uppercase, so "pt-br" is written "pt-BR".
    return "-".join([parts[0].lower()] + [p.upper() if len(p) == 2 else p.lower()
                                          for p in parts[1:]])


def main() -> None:
    ap = argparse.ArgumentParser(description="Publish an adapted piece to ulpia.io/blog/.")
    ap.add_argument("body", help="path to a markdown file holding the piece, without frontmatter")
    ap.add_argument("--title", required=True)
    ap.add_argument("--description", default="")
    ap.add_argument("--date", default=datetime.date.today().isoformat())
    ap.add_argument("--slug", default=None, help="defaults to the title, slugified")
    ap.add_argument("--source", default="", help="the video this came from, recorded in the frontmatter")
    ap.add_argument("--lang", default="",
                    help="BCP 47 tag for <html lang>, such as pt, pt-BR or en. This is the "
                         "language of the post, not of the recording. Omitted, the field is "
                         "left out and the build keeps its own default")
    ap.add_argument("--dry-run", action="store_true", help="write the file, commit nothing")
    args = ap.parse_args()

    # A --lang nobody can parse is a typo by a person, so it stops the run rather
    # than being silently dropped. No --lang at all is not an error and not a
    # guess: the field is left out and the build applies its own default, which is
    # the one place that default is written down.
    lang = None
    if args.lang:
        lang = normalise_lang(args.lang)
        if not lang:
            sys.exit(f"scribe: --lang {args.lang!r} is not a language tag")

    body = Path(args.body).read_text(encoding="utf-8").strip()
    if not body:
        sys.exit("scribe: the body is empty")

    slug = args.slug or slugify(args.title)
    target = POSTS / f"{args.date}-{slug}.md"

    # The date lives in the filename and in the frontmatter, and they are written
    # from the same value here so they cannot drift.
    front = [f"title: {args.title}", f"date: {args.date}"]
    if lang:
        front.append(f"lang: {lang}")
    if args.description:
        front.append(f"description: {args.description}")
    if args.source:
        front.append(f"source: {args.source}")

    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("---\n" + "\n".join(front) + "\n---\n\n" + body + "\n", encoding="utf-8")
    print(f"scribe: wrote {target.relative_to(REPO)}"
          + (f", lang: {lang}" if lang else ", no lang, so the build applies its own default"))

    if args.dry_run:
        print("scribe: dry run, nothing committed")
        return

    # kb commit rather than git commit: more than one session writes this
    # repository, and naming the path is what keeps somebody else's half finished
    # work out of this commit. See decisions/0021.
    kb = REPO / "tools" / "kb" / "target" / "release" / "kb.exe"
    rel = str(target.relative_to(REPO)).replace("\\", "/")
    message = f"Publish: {args.title}\n\nTranscribed and adapted from a recording."
    if args.source:
        message += f"\nSource: {args.source}"

    cmd = [str(kb), "commit", rel, "-m", message] if kb.exists() else \
          ["git", "commit", "--", rel, "-m", message]
    out = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True)
    print(out.stdout.strip() or out.stderr.strip())
    if out.returncode != 0:
        sys.exit("scribe: the commit failed, so nothing was pushed")

    push = subprocess.run(["git", "push"], cwd=REPO, capture_output=True, text=True)
    print(push.stdout.strip() or push.stderr.strip())
    if push.returncode != 0:
        sys.exit("scribe: committed, but the push failed. The piece is in git and not yet live.")
    print("scribe: pushed. The build publishes it.")


if __name__ == "__main__":
    main()
