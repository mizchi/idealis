#!/bin/bash
F=<scratch>/fix1
B=<after>
BS=<before>
tag=$1
A="$BS/crates/idealis-core/target/release/idealis-core $BS/crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm $BS/dist"
$F/run.sh baseline-$tag $B node dist/evals/experiments/nesting-chains/baseline.js $A $F/baseline-$tag.json
$F/run.sh cases-$tag $B node dist/evals/experiments/nesting-chains/generate.js $F/baseline-$tag.json $F/cases-$tag.json
$F/run.sh digests-$tag $B node dist/evals/experiments/nesting-chains/digests.js $A $F/digests-$tag.json
for n in baseline cases digests; do cp $F/$n-$tag.json $B/fixtures/nesting-chains/$n.json; done
