@echo off
REM The local judge for kb-bench longmem: a grading prompt on stdin, yes or no on
REM stdout. Haiku, because judging answer-vs-answer equivalence is classification,
REM not writing.
REM
REM CLEARLY LABELLED: this is NOT the official LongMemEval protocol, which judges
REM with GPT-4o. Scores judged here are internal signal; the harness also writes the
REM official hypotheses JSONL so anyone can re-judge with the official script before
REM comparing against any published number.
claude -p --model claude-haiku-4-5-20251001 --max-turns 1 ^
  --strict-mcp-config --mcp-config "{\"mcpServers\":{}}" ^
  --settings "{\"hooks\":{}}"
