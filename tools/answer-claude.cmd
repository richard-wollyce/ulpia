@echo off
REM The answerer: retrieval's passages on stdin, a grounded answer on stdout.
REM
REM Same process contract as the classifier and the promoters (decisions/0027): any
REM model behind any runtime satisfies it, swap this file and nothing in the binary
REM changes. This model sits AFTER the verdict, never inside retrieval, so the line
REM decisions/0018 draws is untouched.
REM
REM Sonnet, because the job is writing faithful prose from evidence, which is a real
REM writing task, and because benchmark judges grade the wording. Swap to Haiku when
REM cost per answer matters more than polish; the grounding rules live in the prompt
REM kb assembles, not here, so the swap changes the pen and not the rules.
REM
REM The isolations, same three as everywhere in this fleet, and the middle one is
REM load bearing: the answerer must NOT be able to search the base, or its answer
REM stops being grounded in what retrieval served and starts being its own opinion
REM of the library. What it may cite is exactly what arrived on stdin.
claude -p --model claude-sonnet-5 --max-turns 2 ^
  --strict-mcp-config --mcp-config "{\"mcpServers\":{}}" ^
  --settings "{\"hooks\":{}}"
