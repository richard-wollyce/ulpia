# Dossier on stdin, verdict on stdout. The other half of the contract in
# decisions/0027, spoken by a model on this machine instead of one in a datacentre.
#
# Talks to a resident llama.cpp server rather than spawning llama-cli. That is the
# whole latency argument: a 537 MB model loaded once answers in seconds, and the
# same model loaded per message spends most of its time reading itself off disk.
# `kb doctor` reports whether the server is up; nothing here starts it, because a
# hook that boots a server on a cold prompt is a hook that times out.
$ErrorActionPreference = 'Stop'
$port  = if ($env:ULPIA_CLASSIFIER_PORT) { $env:ULPIA_CLASSIFIER_PORT } else { '4115' }
$dossier = [Console]::In.ReadToEnd()

# The grammar is built from the dossier it is about to answer, so the closed list of
# owners is always exactly this fleet's roster. Deriving it here rather than writing
# it down means adding an agent cannot leave a stale grammar behind, and an owner
# that is not on the roster becomes impossible to emit rather than caught afterwards.
$names = @()
foreach ($line in $dossier -split "`n") {
    if ($line -match '^  ([a-z0-9][a-z0-9_-]*)\s*$') { $names += $Matches[1] }
}
if ($names.Count -eq 0) { exit 1 }
$owners = (($names + 'none') | ForEach-Object { '"' + $_ + '"' }) -join ' | '

# SUBJECT and REASON are generated before COVERAGE and OWNER on purpose. A 0.8B
# asked for the name first wrote `OWNER: steve` above a REASON that described Zed:
# it committed to a name and then narrated around it. Naming the subject first
# costs about twenty tokens and gives the choice something to be derived from.
$grammar = @"
root ::= "SUBJECT: " subject "\nREASON: " reason "\nCOVERAGE: " coverage "\nOWNER: " owner "\n"
subject ::= [^\n]{4,60}
reason ::= [^\n]{10,200}
coverage ::= "covered" | "adjacent" | "uncovered"
owner ::= $owners
"@

$body = @{
    messages    = @(@{ role = 'user'; content = $dossier })
    temperature = 0
    max_tokens  = 160
    grammar     = $grammar
    # Qwen3.5 reasons before answering unless told not to. Measured on the DevOps
    # dossier: 13.5s with thinking on, 4.7s with it off, same verdict. The dossier
    # already contains the reasoning this task needs.
    chat_template_kwargs = @{ enable_thinking = $false }
} | ConvertTo-Json -Depth 6 -Compress

try {
    $r = Invoke-RestMethod -Uri "http://127.0.0.1:$port/v1/chat/completions" `
        -Method Post -Body ([Text.Encoding]::UTF8.GetBytes($body)) `
        -ContentType 'application/json' -TimeoutSec 120
} catch {
    # Silence, not a guess. kb falls back to the deterministic choice when the
    # classifier says nothing, and a wrong verdict is worse than no verdict.
    exit 1
}
[Console]::Out.Write($r.choices[0].message.content)
