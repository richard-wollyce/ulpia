"""Put an adapted piece into the site and publish it.

    python tools/scribe/publish.py post.md --title "..." --description "..." [--date YYYY-MM-DD]
    python tools/scribe/publish.py post.md --title "..." --dry-run

Takes a markdown body, writes it into the site's content folder with the
frontmatter the build requires, commits exactly that one file, and pushes, which
is what triggers the deploy.

The division of labour this file assumes: the adaptation from transcript to
written piece is the agent's work and cannot be automated here, because it is
judgement about a voice. What can be automated is everything after that
judgement, and everything after it is mechanical enough that doing it by hand
invites the two mistakes this file exists to prevent: a filename that disagrees
with the date in the frontmatter, and a commit that carries somebody else's
work because the paths were not named.
"""

import argparse
import datetime
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
POSTS = REPO / "site" / "frontend" / "content" / "posts"


def slugify(text: str) -> str:
    text = re.sub(r"[^\w\s-]", "", text, flags=re.UNICODE).strip().lower()
    return re.sub(r"[\s_-]+", "-", text)[:70]


def main() -> None:
    ap = argparse.ArgumentParser(description="Publish an adapted piece to ulpia.io/blog/.")
    ap.add_argument("body", help="path to a markdown file holding the piece, without frontmatter")
    ap.add_argument("--title", required=True)
    ap.add_argument("--description", default="")
    ap.add_argument("--date", default=datetime.date.today().isoformat())
    ap.add_argument("--slug", default=None, help="defaults to the title, slugified")
    ap.add_argument("--source", default="", help="the video this came from, recorded in the frontmatter")
    ap.add_argument("--dry-run", action="store_true", help="write the file, commit nothing")
    args = ap.parse_args()

    body = Path(args.body).read_text(encoding="utf-8").strip()
    if not body:
        sys.exit("scribe: the body is empty")

    slug = args.slug or slugify(args.title)
    target = POSTS / f"{args.date}-{slug}.md"

    # The date lives in the filename and in the frontmatter, and they are written
    # from the same value here so they cannot drift.
    front = [f"title: {args.title}", f"date: {args.date}"]
    if args.description:
        front.append(f"description: {args.description}")
    if args.source:
        front.append(f"source: {args.source}")

    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("---\n" + "\n".join(front) + "\n---\n\n" + body + "\n", encoding="utf-8")
    print(f"scribe: wrote {target.relative_to(REPO)}")

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
