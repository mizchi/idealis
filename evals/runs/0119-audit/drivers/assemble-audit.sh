#!/bin/bash
# usage: E=<scratch> W=<worktree> assemble-audit.sh
: "${E:?}"; : "${W:?}"
export E W
cd "$W" || exit 1
A=$E/audit-src
rm -rf "$A"; mkdir -p "$A/logs" "$A/superseded" "$A/checks" "$A/drivers"
cp "$E/commands.tsv" "$A/"
for f in run.sh build-side.sh build-harness.sh peaks.py boundary.py ceiling-depths.py split-chromium.py summarize-stack.py summarize-wasm.py record.py assemble-runs.sh assemble-audit.sh mkparity.py copy-bench-into-deps.sh copy-bench-to-deps.sh paths.mjs tlc.mjs analyze-paths.py analyze-tlc.py protected.sh wasm-node.sh stage256.sh loops.sh slot-a.sh slot-b.sh slot-c.sh slot-d.sh slot-e.sh slot-f.sh master.sh master2.sh final2.sh after-before.sh; do
  [ -e "$E/drivers/$f" ] && cp "$E/drivers/$f" "$A/drivers/"
done
cp "$E"/logs/*.exit "$A/logs/"
for pat in 'red*' 'green*' 'release*' 'oracle-rtlc*' 'oracle-parallel*' 'size-*' 'final-*' 'check-*' 'just-*' 'corpus*' 'temporal-parity*' 'kernel*' 'freeze-*' 'baseline-*' 'expect-*' 'paths-*' 'analyze-*' 'tlc-0119*' 'wasm-*' 'cli-*' 'stack256-*' 'stack-oracle*' 'stack-after*' 'stack-before*'; do
  for f in "$E"/logs/$pat.log; do
    [ -e "$f" ] || continue
    case "$f" in */final-mutation*|*/final-check*|*/final-safety*|*/final-portable-test*|*/final-go-test*|*/check-rtlc*|*/check-compiler*|*/release3-rtlc-all*|*/release3-compiler-all*|*/release-rtlc-all*|*/release-compiler-all*) tail -n 60 "$f" > "$A/logs/$(basename "$f")";; *) cp "$f" "$A/logs/";; esac
  done
done
cp "$E/scratch/protected-before.txt" "$A/checks/protected-before.txt"
cp "$E/scratch/protected-after-final.txt" "$A/checks/protected-after.txt"
cp "$E/scratch/oracle.patch" "$A/checks/oracle-first-build.patch" 2>/dev/null
for s in before-r2 after oracle oracle-r2; do [ -e "$E/snapshots/$s/manifest.json" ] && cp "$E/snapshots/$s/manifest.json" "$A/checks/snapshot-$s.json"; [ -e "$E/snapshots/$s/sha256.txt" ] && cp "$E/snapshots/$s/sha256.txt" "$A/checks/snapshot-$s.sha256"; done
(cd "$W" && git status --short) > "$A/checks/final-git-status.txt"
shasum -a 256 "$E/tools/tla2tools.jar" "$E/tools/apalache-0.63.0.jar" "$E/pinned/apalache-0.63.0.tgz" | sed "s#$E/##" > "$A/checks/tool-digests.txt"
cp "$E/runs/paths-r1/oracle.json" "$A/superseded/paths-r1-oracle.json" 2>/dev/null
for f in "$E"/runs/stack/after-*.json "$E"/runs/stack/before-*.json "$E"/runs/stack/oracle-*.json; do cp "$f" "$A/superseded/stack-512-$(basename "$f")"; done
for f in oracle-node-default-r2 oracle-node-liftoff-only-r2 oracle-node-no-liftoff-r2 oracle-chromium-r2 oracle-node-depths oracle-chromium-depths; do cp "$E/runs/wasm/$f.json" "$A/superseded/wasm-first-$f.json"; done
cp "$E/runs/oracle/baseline-before-0119.json" "$A/superseded/baseline-before-0119-r1.json"; cp "$E/runs/oracle/baseline-oracle-0119.json" "$A/superseded/baseline-oracle-0119-r1.json"
cp "$E/scratch/audit-readme.md" "$A/README.md"
rm -rf "$W/evals/runs/0119-audit"
python3 -I "$E/drivers/record.py" "$A" "$W/evals/runs/0119-audit" '*'
du -sh "$W/evals/runs/0119-audit"
