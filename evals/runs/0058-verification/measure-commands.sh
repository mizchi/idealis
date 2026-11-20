#!/bin/zsh
set -eu
IDEALIS_PARITY_PLAN=0058-packed-hash IDEALIS_PARITY_EXTRA=evals/compact-functions/boundaries.json node dist/evals/temporal-tuning/parity.js .artifacts/0058-parity-before evals/runs/0058-parity-verified > .artifacts/0058-parity-verified.log 2>&1
node dist/evals/oracle-tuning/kernel.js .artifacts/0058-before .artifacts/0058-after evals/runs/0058-criterion safety-bench > .artifacts/0058-criterion.log 2>&1
IDEALIS_COMPACT_PLAN=0058 node dist/evals/compact-functions/compare.js .artifacts/0058-before .artifacts/0058-after .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar .artifacts/tlc/jre.tar.gz evals/runs/0058-comparison > .artifacts/0058-comparison.log 2>&1
node dist/evals/compact-functions/summarize.js evals/runs/0058-comparison evals/runs/0058-criterion evals/runs/0058-summary > .artifacts/0058-summary.log 2>&1
