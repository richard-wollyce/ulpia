@echo off
REM The official LongMemEval re-judgement, run exactly as the paper's protocol runs it:
REM their evaluate_qa.py, unmodified, judging our hypotheses with gpt-4o against the
REM reference file. Requires OPENAI_API_KEY in the environment, set by the OWNER, never
REM handled by an agent.
cd /d %~dp0
python evaluate_qa.py gpt-4o hypotheses-s.jsonl data\longmemeval_s_cleaned.json
