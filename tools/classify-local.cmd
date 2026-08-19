@echo off
REM The local half of the classifier contract. See classify-local.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0classify-local.ps1"
