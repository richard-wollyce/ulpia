@echo off
REM Promoter one: a deposit on stdin, zero or more PROPOSAL blocks on stdout.
REM
REM Deliberately a script and not Rust, for the same reason as classify-claude.cmd:
REM the contract is a process, so any model behind any runtime can hold up its end.
REM
REM Sonnet, not Haiku and not Opus. The job is to read a pile of unreviewed material
REM and write a note with thirty to seventy keys in two languages, which is a writing
REM task with real judgement in it and more than Haiku should be asked for. It is not
REM the deciding step, and the deciding step is where the stronger model goes: see
REM review-claude.cmd and decisions/0030.
REM
REM Three isolations, all load bearing:
REM
REM   --settings with no hooks. Without it this inherits the fleet's UserPromptSubmit
REM   hook, which runs kb boot, which runs a model: a regress that looks like a hang.
REM
REM   --strict-mcp-config with no servers. Promoter one must NOT be able to search the
REM   base. Proposing and checking are different jobs and this half only proposes; if
REM   it could read the base it would become the reviewer's input too, and the second
REM   reader would stop being independent.
REM
REM   --max-turns 1. One pass over the deposit. A promoter that can iterate is a
REM   promoter that can talk itself into keeping something.
claude -p --model claude-sonnet-5 --max-turns 1 ^
  --strict-mcp-config --mcp-config "{\"mcpServers\":{}}" ^
  --settings "{\"hooks\":{}}"
