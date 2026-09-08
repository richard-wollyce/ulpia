# MAP: the knowledge base map

> The first file to read on any query, after `index.md`. It says what exists, where it
> lives, and what connects to what.
>
> Link convention: `[[file-name]]`, no extension. One subject per file, descriptive
> kebab-case names. **Every new file gets an entry here, in the same move that creates
> it. A file nobody can find does not exist.**

---

## Folder structure

| Folder | What goes here |
|---|---|
| `knowledge/` | Distillations by domain. **The brain** |
| `inbox/` | Raw material awaiting distillation |
| `decisions/` | Decisions that outlive a conversation |
| `protocols/` | This agent's own procedures |
| `templates/` | Document skeletons |

---

## Current contents

Nothing yet. Skeleton was created by `kb init` and has not been fed.

An entry here is a reading list line: the `[[wikilink]]` and one sentence saying what
the file is. **The keys do not go here.** They go in the note's own header, on a
`**Search for:**` line above the first `##`, which is the only copy the router reads
and the copy `kb check` requires. `kb write` puts them there for you. Two copies of
one key list drift, and the copy nobody reads is the one that drifts first.
