#!/bin/zsh
set -eu
IDEALIS_PARITY_PLAN=0057-probe-updates IDEALIS_PARITY_EXTRA=evals/compact-functions/boundaries.json node dist/evals/temporal-tuning/parity.js .artifacts/0057-parity-before evals/runs/0057-parity-b > .artifacts/0057-parity-b.log 2>&1
node dist/evals/oracle-tuning/kernel.js .artifacts/0057-before .artifacts/0057-after-b evals/runs/0057-criterion-b safety-bench > .artifacts/0057-criterion-b.log 2>&1
IDEALIS_COMPACT_PLAN=0057 node dist/evals/compact-functions/compare.js .artifacts/0057-before .artifacts/0057-after-b .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar .artifacts/tlc/jre.tar.gz evals/runs/0057-comparison-b > .artifacts/0057-comparison-b.log 2>&1
node dist/evals/compact-functions/summarize.js evals/runs/0057-comparison-b evals/runs/0057-criterion-b evals/runs/0057-summary-b > .artifacts/0057-summary-b.log 2>&1
