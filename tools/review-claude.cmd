@echo off
REM Promoter two: one proposal plus the router's evidence on stdin, a two line
REM verdict on stdout. Called once per lens, so three times per proposal.
REM
REM Opus, and this is where the money goes. Promoter one writes and this one
REM decides, and a note written badly costs one note while a note admitted wrongly
REM costs every question it later wins. Richard's instruction was explicit: the
REM reviewer is the more competent reader.
REM
REM Three isolations, and the middle one is the whole design:
REM
REM   --settings with no hooks, same regress as everywhere else in this fleet.
REM
REM   --strict-mcp-config with no servers. **The reviewer must not be able to search
REM   the base.** It is handed what the base holds by kb itself, from the
REM   deterministic router, with no model in that path. If it could search, its view
REM   of the base would become another model's opinion and the independence that
REM   makes a second reader worth running would be gone. This flag is not hygiene
REM   here, it is the mechanism.
REM
REM   --max-turns 1. It answers the one question it was given. A reviewer that can
REM   take a second turn is a reviewer that can be argued around.
claude -p --model claude-opus-5 --max-turns 1 ^
  --strict-mcp-config --mcp-config "{\"mcpServers\":{}}" ^
  --settings "{\"hooks\":{}}"
