#!/bin/bash
E=<scratch>; W=<after>; A=$E/audit-src
rm -rf $A; mkdir -p $A/logs $A/superseded $A/checks $A/drivers
cp $E/commands.tsv $A/; cp $E/drivers/* $A/drivers/; cp $E/logs/*.exit $A/logs/; cp $E/logs/*.note $A/logs/ 2>/dev/null
for f in $E/logs/red-*.log $E/logs/green-*.log $E/logs/check-*.log $E/logs/just-*.log $E/logs/tlc-0075*.log $E/logs/bench-abba-*.log $E/logs/kernel*.log $E/logs/corpus*.log $E/logs/temporal-parity*.log $E/logs/freeze-*.log $E/logs/copy-*.log $E/logs/baseline-*.log $E/logs/safety-build-*.log $E/logs/expect-*.log $E/logs/paths-*.log $E/logs/stack-kinds-*.log $E/logs/stack-chains-*.log; do cp "$f" $A/logs/; done
cp $E/audit/checks/* $A/checks/; cp $E/audit/ledger-appended.diff $A/
python3 -I $E/drivers/analyze-0075-paths.py $E/runs/paths $W/fixtures/nesting-chains/cases.json $W/fixtures/nesting-chains/cases-0075.json > /dev/null
cp $E/runs/paths/analysis.txt $A/superseded/paths-r1-analysis.txt; cp $E/runs/paths-r2/analysis.txt $A/superseded/paths-r2-analysis.txt
cp $E/runs/tlc/results.json $A/superseded/tlc-r1-results.json; cp $E/runs/stack/summary.md $A/superseded/stack-r1-summary.md
for p in probe-defs probe-key abba-3-probe; do cp $E/runs/$p/table.txt $A/superseded/$p-table.txt; done
cp $E/runs/kernel-summary/kernel-table.txt $A/superseded/kernel-r1-table.txt
cp $E/snapshots/red-tests.sha256 $A/checks/; cp $E/snapshots/bench-manifest*.json $A/checks/
for s in before-r4 after-r4 after-r5 oracle-r4; do cp $E/snapshots/$s/manifest.json $A/checks/snapshot-$s.json; done
(cd $W && git status --short) > $A/checks/final-git-status.txt
du -sh $A; du -a $A | sort -n | tail -6
