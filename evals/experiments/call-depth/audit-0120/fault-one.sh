#!/bin/bash
# usage: fault-one.sh <id>
source <scratch>/env.sh
id=$1
export S W
python3 -I $W/evals/experiments/call-depth/audit-0120/faults.py "$id" || exit 90
$S/run --timeout 1800 fault-$id $S/trees/fault cargo test --locked -j 4 --manifest-path crates/rtlc/Cargo.toml --lib --target-dir $S/target-fault
code=$?
grep -E '^test .* \.\.\. FAILED$' $S/logs/fault-$id.log | sed -E 's/^test (.*) \.\.\. FAILED$/\1/' | sort > $S/runs/faults/$id.failed.txt
grep -E '^test result' $S/logs/fault-$id.log > $S/runs/faults/$id.result.txt
grep -c "Compiling rtlc" $S/logs/fault-$id.log > $S/runs/faults/$id.compiled.txt
echo "$id: exit=$code failed=$(wc -l < $S/runs/faults/$id.failed.txt) $(cat $S/runs/faults/$id.result.txt | head -1)"
python3 -I $W/evals/experiments/call-depth/audit-0120/faults.py baseline > <null-device>
