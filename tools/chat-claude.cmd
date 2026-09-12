@echo off
REM The reading room's conversation: a message on stdin, stream-json on stdout.
REM
REM Split out of ui.rs on 2026-09-12, so this call site follows decisions/0027 like the
REM other four. It was the last one that named a runtime in Rust, and it was the one most
REM likely to be copied into the desktop shell.
REM
REM The contract here is NARROWER than the classifier's, and the difference is the whole
REM reason this file exists rather than a flag:
REM
REM   stdout is one JSON object per line, in Claude Code's `stream-json` shape. kb reads
REM   the session id out of that stream and hands it back on the next message.
REM
REM   the session to resume arrives in the environment as KB_CHAT_RESUME, never on the
REM   command line. kb no longer builds the argument list, so "nothing from outside is
REM   ever an argument" is the only rule that survives somebody editing fleet.txt.
REM
REM A runtime that does not speak that shape needs a translating wrapper here, not a
REM different flag. That is the honest state of it: the contract is a process, and this
REM particular process still speaks one vendor's dialect.
REM
REM No isolation flags, and that is deliberate: unlike the classifier, the promoters and
REM the answerer, this one IS the agent. It is supposed to have its hooks, its MCP servers
REM and the fleet's constitution, because the person is talking to it.
if defined KB_CHAT_RESUME (
  claude -p --output-format stream-json --verbose --resume %KB_CHAT_RESUME%
) else (
  claude -p --output-format stream-json --verbose
)
