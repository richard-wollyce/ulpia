@echo off
REM Turns a PDF into the text `kb ingest` distils from. One argument in, text on stdout.
REM
REM This is the whole extractor contract, and it is the same process contract the
REM classifier and both promoters already use: a command named by convention, the
REM document as argv[1], the result on stdout, a non zero exit when it could not.
REM `kb ingest` looks for tools\extract\<extension>.cmd and says so by name when the
REM file is not there, so adding a format is writing one of these and nothing else.
REM
REM **Shelling out rather than linking is the decision.** `kb` has one dependency and
REM keeping it there is worth more than the convenience of a PDF crate: a parser for
REM every format anybody might hand over is an unbounded surface, the formats change,
REM and a bad parser is a crash inside the tool rather than a non zero exit outside it.
REM
REM -layout keeps columns and tables readable, which matters more than it sounds: the
REM chunker splits on markdown headings and falls back to one chunk per document when
REM there are none, so a PDF that comes out as one wall of reflowed prose is a single
REM chunk that no passage can be quoted from.
REM
REM -enc UTF-8 is not optional for anything in Portuguese. Without it pdftotext emits
REM the platform codepage and every accented word arrives mangled, which was measured on
REM 2026-09-05: the first extraction of a Brazilian textbook came back with U+FFFD where
REM every "ç" and "ã" had been.
REM
REM The trailing - means stdout. pdftotext writes a file otherwise.
REM
REM Needs poppler. It ships with Git for Windows at /mingw64/bin/pdftotext, and is
REM `poppler-utils` on Debian and `poppler` in Homebrew.
pdftotext -layout -enc UTF-8 %1 -
