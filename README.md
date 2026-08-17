# Vesta

A local first fleet of agents, each with its own knowledge base, and one memory layer
that serves all of them.

Your files stay on your machine. `kb` reads plain markdown, derives an index it can
throw away, and answers which files a question should open. There is no model inside
it, and there never will be: retrieval that depends on a model is retrieval you cannot
run offline, cannot audit, and cannot explain when it is wrong.

## What is here

| Path | What it is |
|---|---|
| `tools/kb` | The memory layer. Lints, indexes, routes, and serves MCP. One dependency. |
| `tools/tray` | A Windows tray app over the same library. |
| `agents/` | Your agents. **Not in this repository**, by design. |
| `fleet.txt` | The fleet's name, and the manifest for what convention cannot express. |

## Your knowledge is not in here

`agents/` is ignored by this repository and is a separate one of its own. That is
structural, not a convention: `git add -A` at this root cannot descend into it, so
publishing a note is not a mistake you are able to make.

To start a fleet of your own:

```
kb init myagent
```

That writes the full agent shape, initialises git, and makes the first commit, so the
agent it creates can be opened by the system that created it.

## Using it

```
kb check .            lint every agent
kb index .            build one index per agent
kb route "question" . which files should this open
kb fleet .            who is in the fleet
kb serve .            speak MCP over stdio, for Claude Desktop and others
```

## Design record

Decisions that outlive a conversation are written down as ADRs, with the mechanism that
makes each one work and the cost it accepts. They live with the architect agent rather
than here, for now.

## Licence

Not chosen yet.
