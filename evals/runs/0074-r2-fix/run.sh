#!/bin/bash
# usage: run.sh <name> <cwd> <cmd...>
F=<scratch>/fix1
name=$1; cwd=$2; shift 2
s=$(date -u +%FT%TZ)
cd "$cwd"
"$@" > "$F/logs/$name.log" 2>&1
code=$?
e=$(date -u +%FT%TZ)
printf '%s\t%s\t%s\t%s\tcwd=%s\texit=%s\n' "$name" "$s" "$e" "$*" "$cwd" "$code" >> "$F/commands.tsv"
echo "$name exit=$code"
exit 0
