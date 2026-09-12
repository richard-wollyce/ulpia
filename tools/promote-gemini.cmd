@echo off
REM Promoter one: a deposit on stdin, zero or more PROPOSAL blocks on stdout.
REM
REM The Gemini half of the contract. **Read classify-gemini.cmd first**: it carries the
REM whole explanation of why the isolation is a settings file here and not flags, and the
REM `-e none` trap that silently re-enables extensions if a second -e is ever appended.
REM This file is that one with a different model and nothing else.
REM
REM Model: gemini-2.5-flash.
REM Flash: this is a writing job from evidence already in the prompt, not a reasoning job.
REM
REM **Not run.** Written from the CLI's source and docs on 2026-09-12, never executed.

setlocal
if "%GEMINI_API_KEY%"=="" (
  echo promote-gemini: GEMINI_API_KEY is not set in the environment. 1>&2
  exit /b 1
)
set "GEMINI_CLI_SYSTEM_SETTINGS_PATH=%~dp0gemini-judge-settings.json"
gemini --model gemini-2.5-flash --extensions none
