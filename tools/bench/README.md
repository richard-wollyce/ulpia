# kb-bench

Retrieval measurement against a real fleet, in Rust, so re-checking ADR-0017 and
ADR-0018 is one command instead of a Python environment. `kb-bench --help` for the
modes. The full decision table it produced lives in ADR-0018.

Models download on first use into `%LOCALAPPDATA%/kb-bench` (override with
`KB_BENCH_CACHE`) and were deleted after the review; re-running re-downloads.

## Windows note: hf-hub 0.5.0 kills its own first download

The crate creates a relative symlink from the snapshot to the blob, the Windows
API then fails to resolve it, and `assert!(pointer_path.exists())` panics after
the bytes are already on disk. Workaround: materialise real files in the layout
hf-hub expects, and it will never try to download or link again.

```
sha=$(curl -sL https://huggingface.co/api/models/<repo> | python -c "import sys,json;print(json.load(sys.stdin)['sha'])")
dir="$LOCALAPPDATA/kb-bench/models--<org>--<name>"
mkdir -p "$dir/snapshots/$sha" "$dir/refs" && printf "%s" "$sha" > "$dir/refs/main"
curl -sL https://huggingface.co/<repo>/resolve/main/<file> -o "$dir/snapshots/$sha/<file>"
```

Files each model needs: its `model_file` from fastembed's model list (plus
`model.onnx.data` for bge-reranker-v2-m3), and `tokenizer.json`, `config.json`,
`special_tokens_map.json`, `tokenizer_config.json`.
