#!/bin/bash
# usage: run.sh <name> <cwd> <cmd...>   appends to commands.tsv, logs to logs/<name>.log
E=<scratch>/eval-0073-r3
name=$1; cwd=$2; shift 2
log=$E/logs/$name.log
[ -e "$log" ] && { echo "refusing to overwrite $log" >&2; exit 99; }
start=$(date -u +%FT%TZ)
load=$(uptime | sed 's/.*load averages*: //')
cd "$cwd"
"$@" >"$log" 2>&1
code=$?
end=$(date -u +%FT%TZ)
printf '%s\t%s\t%s\t%s\tcwd=%s\tload_at_start=%s\texit=%s\t%s\n' "$name" "$start" "$end" "$*" "$cwd" "$load" "$code" "" >> $E/commands.tsv
echo "$name exit=$code" > $E/logs/$name.exit
exit $code
