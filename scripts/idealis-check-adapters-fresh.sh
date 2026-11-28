#!/bin/sh
# Plan 0108: the recorded outputs of the Pkl adapter (and of its two
# artificial faults) are what the adapter writes now.
set -eu
pkl_dir=evals/experiments/surface-languages/pkl
status=0
for variant in "" swap-insert-remove flip-join; do
  dir=fixtures/rcheck/adapters/pkl${variant:+-$variant}
  for model in permissions bank order sessions service; do
    fresh=$(ADAPTER_FAULT="$variant" pkl eval "$pkl_dir/ir-$model.pkl")
    if [ "$fresh" != "$(cat "$dir/$model.json")" ]; then
      echo "$dir/$model.json is stale: rerun the Pkl adapter" >&2
      status=1
    fi
  done
done
exit "$status"
