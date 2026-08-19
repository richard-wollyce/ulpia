@echo off
REM The reference classifier: dossier on stdin, verdict on stdout.
REM
REM Deliberately a script and not Rust. The contract in decisions/0027 is a
REM process, so any model behind any runtime can hold up its end: replace this
REM file with one that calls a local llama.cpp, ollama, an API or a person, and
REM nothing in the binary changes.
REM
REM Haiku, because the task is to pick one name from a list of four given a
REM roster and five lines of evidence. Spending a frontier model on that buys
REM reasoning the task does not contain.
REM
REM Two isolations, and both are load bearing:
REM
REM   --settings with no hooks. Without it the classifier inherits the fleet's
REM   UserPromptSubmit hook, which runs kb boot, which runs the classifier: an
REM   infinite regress that would look like a hang.
REM
REM   --strict-mcp-config with no servers. The classifier must not be able to
REM   search the base. If it can, it stops being a classifier and becomes a
REM   second agent with opinions, and the deterministic retrieval it is supposed
REM   to be judging becomes advisory.
claude -p --model claude-haiku-4-5-20251001 --max-turns 1 ^
  --strict-mcp-config --mcp-config "{\"mcpServers\":{}}" ^
  --settings "{\"hooks\":{}}"
