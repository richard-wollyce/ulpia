@echo off
REM Promoter two: one proposal plus the router's evidence on stdin, a two line verdict on stdout.
REM
REM The Gemini half of the contract. **Read classify-gemini.cmd first**: it carries the
REM whole explanation of why the isolation is a settings file here and not flags, and the
REM `-e none` trap that silently re-enables extensions if a second -e is ever appended.
REM This file is that one with a different model and nothing else.
REM
REM Model: gemini-2.5-pro.
REM Pro rather than Flash, and this is where the money goes, for the same reason
REM review-claude.cmd uses Opus: promoter one writes and this one decides. A note written
REM badly costs one note; a note admitted wrongly costs every question it later wins.
REM
REM It must also be a DIFFERENT model from the promoter. `kb promote` refuses to run when
REM either line is absent, but nothing stops you pointing both at the same model, and that
REM would quietly delete the independence the two-reader design exists for.
REM
REM **Not run.** Written from the CLI's source and docs on 2026-09-12, never executed.

setlocal
if "%GEMINI_API_KEY%"=="" (
  echo review-gemini: GEMINI_API_KEY is not set in the environment. 1>&2
  exit /b 1
)
set "GEMINI_CLI_SYSTEM_SETTINGS_PATH=%~dp0gemini-judge-settings.json"
gemini --model gemini-2.5-pro --extensions none
