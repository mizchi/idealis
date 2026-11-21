#!/bin/bash
D=<scratch>/eval-0073-r3/runs/0073-r3-corpus
for v in v1 v2; do
  for f in "$D"/*-1-after-native-$v.json; do
    python3 -I <scratch>/eval-0073-r3/drivers/scan-dead-binds.py "$f" 2>&1 | tail -1
  done
done
