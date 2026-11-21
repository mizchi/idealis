#!/bin/bash
# usage: timed.sh <name> <cwd> <cmd...>; samples load and top CPU consumers every 5 s into logs/<name>.load
E=<scratch>/eval
name=$1
( while true; do echo "$(date -u +%FT%TZ) $(uptime | sed 's/.*load averages*: //') | $(ps -Ao pcpu,comm | sort -nr | sed -n '1,4p' | awk '{printf "%s%% %s; ", $1, $2}' | sed 's#/[^ ;]*/##g')" >> $E/logs/$name.load; sleep 5; done ) &
sampler=$!
"$E/drivers/run.sh" "$@"
code=$?
kill $sampler 2>/dev/null
exit $code
