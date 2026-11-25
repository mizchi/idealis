#!/bin/bash
# usage: E=<scratch> W=<worktree> assemble-runs.sh
# Copies the cited summaries of each run into evals/runs/0119-* with host paths replaced by placeholders.
: "${E:?}"; : "${W:?}"
export E W
cd "$W" || exit 1
rec() { python3 -I "$E/drivers/record.py" "$@"; }
O=evals/runs

# stack: the final (limit 256) measurements and the generated summary
rm -rf "$E/scratch/stack-src"; mkdir -p "$E/scratch/stack-src"
cp "$E"/runs/stack/256-*.json "$E/scratch/stack-src/"
python3 -I "$E/drivers/summarize-stack.py" "$E/runs/stack" "$E/scratch/stack-src/summary.md" > /dev/null
rec "$E/scratch/stack-src" "$O/0119-stack" '256-*.json' 'summary.md'

rec "$E/runs/paths" "$O/0119-paths" analysis.txt differences.tsv
rec "$E/runs/tlc" "$O/0119-tlc" analysis.txt verdicts.json tlc/results.json
rec "$E/runs/corpus" "$O/0119-corpus" manifest.json plan.md result.json ts-frontend.json
rec "$E/runs/corpus-summary" "$O/0119-corpus-summary" result.json
rec "$E/runs/temporal-parity" "$O/0119-temporal-parity" result.json
rec "$E/runs/kernel" "$O/0119-kernel" manifest.json inputs.json
rec "$E/runs/kernel-summary" "$O/0119-kernel-summary" result.json
rec "$E/runs/evaluate" "$O/0119-evaluate" 'summary.json' 'witnesses.json'

# wasm ceilings (oracle rebuilt from the final source, and after), the Go probe and the CLI attempt
rm -rf "$E/scratch/wasm-src"; mkdir -p "$E/scratch/wasm-src"
cp "$E/runs/wasm/summary.md" "$E"/runs/wasm/oracle-r2-*.json "$E"/runs/wasm/oracle-r2-*.txt "$E"/runs/wasm/after-*.json "$E"/runs/wasm/after-go.txt "$E/scratch/wasm-src/" 2>/dev/null
cp "$E/runs/cli/after.json" "$E/scratch/wasm-src/cli-after.json"; cp "$E/runs/cli/oracle.json" "$E/scratch/wasm-src/cli-oracle.json"
rec "$E/scratch/wasm-src" "$O/0119-wasm-ceiling" '*.json' '*.txt' 'summary.md'

# oracle
cp "$E/runs/oracle/baseline-before-0119-r4.json" "$E/records/0119-oracle/baseline-before-0119-r4.json"
cp "$E/runs/oracle/baseline-oracle-0119-r4.json" "$E/records/0119-oracle/baseline-oracle-0119-r4.json"
cp "$E/snapshots/oracle-r2/sha256.txt" "$E/records/0119-oracle/snapshot-oracle.sha256"
rec "$E/records/0119-oracle" "$O/0119-oracle" '*'
