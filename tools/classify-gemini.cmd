@echo off
REM The classifier on Google's Gemini CLI: dossier on stdin, verdict on stdout.
REM
REM Same contract as classify-claude.cmd and decisions/0027. What differs is everything
REM about how the isolation is bought, and that difference is the reason this file has a
REM long comment instead of three lines.
REM
REM ## The isolations are not flags here
REM
REM classify-claude.cmd buys them with --strict-mcp-config, --mcp-config "{}" and
REM --settings "{hooks:{}}". **The Gemini CLI has none of those flags, and no --max-turns
REM either.** Read from its yargs option table at v0.59.0 on 2026-09-12; all four are
REM absent. So the same two properties have to come from a settings file, and the file has
REM to sit at a precedence tier the project cannot override.
REM
REM That tier is the system settings file, and GEMINI_CLI_SYSTEM_SETTINGS_PATH names it.
REM The documented merge order, lowest to highest, is: schema defaults, system defaults,
REM user settings, PROJECT settings, SYSTEM settings, environment, command line. So a
REM `.gemini/settings.json` inside the fleet cannot re-enable what this turns off, which
REM is the whole point: the base being judged must not be able to configure its judge.
REM
REM ## What each setting is standing in for
REM
REM   hooksConfig.enabled: false     the --settings {hooks:{}} of the Claude version. The
REM                                  regress is identical: SessionStart fires a hook, the
REM                                  fleet's hook runs kb boot, kb boot runs this.
REM
REM   tools.core: []                 the --strict-mcp-config half, and it is load bearing.
REM                                  Gemini's default policy lets read-only tools run
REM                                  unprompted, so without this the classifier can read
REM                                  the base it is supposed to be judging and stops being
REM                                  a classifier. An empty array is truthy in JavaScript,
REM                                  which is what makes the CLI's own "if (coreTools)"
REM                                  branch take and then match nothing.
REM
REM   context.fileName               points at a file that exists nowhere, so no GEMINI.md
REM                                  from the working directory reaches the model, and
REM                                  includeDirectoryTree: false stops the cwd listing that
REM                                  ships by default.
REM
REM   model.maxSessionTurns: 1       the --max-turns 1. Exit code 53 on breach.
REM
REM ## The one flag that IS load bearing, and its trap
REM
REM `-e none` disables every extension. The CLI's check requires the override list to be
REM EXACTLY one element long: `-e none -e foo` skips the disabling branch entirely and
REM enables foo, while the command line still reads as though extensions were off. So
REM `-e none` must be the only -e on the line. Never append another.
REM
REM ## Model
REM
REM Flash, for the same reason the Claude version uses Haiku: the job is to pick one name
REM from a list of four given a roster and five lines of evidence. A concrete id and never
REM an alias, because `auto` and `pro` resolve to different models depending on whether
REM preview features are on, and a classifier whose model moves under it is a classifier
REM whose behaviour nobody can reproduce.
REM
REM ## Not run
REM
REM **Nothing in this file has been executed.** It was written from the CLI's source and
REM docs, not from a run, because the Gemini CLI is not installed on this machine. Before
REM trusting it with a real base, run this once and confirm the answer is a refusal and
REM that no tool_use event ever appears:
REM
REM   echo Read tools/kb/src/memory.rs and tell me its first line. | tools\classify-gemini.cmd
REM
REM If it reads the file, tools.core did not take and this file is not isolating anything.

setlocal
if "%GEMINI_API_KEY%"=="" (
  echo classify-gemini: GEMINI_API_KEY is not set in the environment. 1>&2
  exit /b 1
)
set "GEMINI_CLI_SYSTEM_SETTINGS_PATH=%~dp0gemini-judge-settings.json"
gemini --model gemini-2.5-flash --extensions none
