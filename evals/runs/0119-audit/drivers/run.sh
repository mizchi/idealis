#!/bin/bash
# usage: E=<scratch> run.sh <name> <cwd> <cmd...>
# Appends one row to commands.tsv and logs to logs/<name>.log; refuses to overwrite a log.
: "${E:?E must name the scratch directory}"
name=$1; cwd=$2; shift 2
log=$E/logs/$name.log
[ -e "$log" ] && { echo "refusing to overwrite $log" >&2; exit 99; }
start=$(date -u +%FT%TZ)
load=$(uptime | sed 's/.*load averages*: //')
cd "$cwd" || exit 98
"$@" >"$log" 2>&1
code=$?
end=$(date -u +%FT%TZ)
printf '%s\t%s\t%s\t%s\tcwd=%s\tload_at_start=%s\texit=%s\n' "$name" "$start" "$end" "$*" "$cwd" "$load" "$code" >> "$E/commands.tsv"
echo "$name exit=$code" > "$E/logs/$name.exit"
exit $code
