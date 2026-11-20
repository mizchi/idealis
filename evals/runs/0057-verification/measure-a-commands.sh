#!/bin/zsh
set -eu
IDEALIS_PARITY_PLAN=0057-probe-updates IDEALIS_PARITY_EXTRA=evals/compact-functions/boundaries.json node dist/evals/temporal-tuning/parity.js .artifacts/0057-parity-before evals/runs/0057-parity > .artifacts/0057-parity.log 2>&1
node dist/evals/oracle-tuning/kernel.js .artifacts/0057-before .artifacts/0057-after evals/runs/0057-criterion safety-bench > .artifacts/0057-criterion.log 2>&1
IDEALIS_COMPACT_PLAN=0057 node dist/evals/compact-functions/compare.js .artifacts/0057-before .artifacts/0057-after .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar .artifacts/tlc/jre.tar.gz evals/runs/0057-comparison > .artifacts/0057-comparison.log 2>&1
node dist/evals/compact-functions/summarize.js evals/runs/0057-comparison evals/runs/0057-criterion evals/runs/0057-summary > .artifacts/0057-summary.log 2>&1
