@echo off
REM The SECOND judge, for validating the first one. Same contract, same prompt, a
REM different model: a grading prompt on stdin, yes or no on stdout.
REM
REM Sonnet, because independence is the whole point of a second judge and a second
REM Haiku call would mostly measure sampling noise in one model. This is a weaker
REM independence claim than a second vendor's model would be: both judges share the
REM Claude CLI's system prompt and the same pretraining lineage, so a shared blind
REM spot stays invisible to this test. Stated here rather than in a footnote.
REM
REM CLEARLY LABELLED: neither judge is the official LongMemEval protocol, which judges
REM with GPT-4o. This validates our judge against a second one; it does not make either
REM of them official.
claude -p --model claude-sonnet-5 --max-turns 1 ^
  --strict-mcp-config --mcp-config "{\"mcpServers\":{}}" ^
  --settings "{\"hooks\":{}}"
