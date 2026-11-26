#!/bin/bash
# usage: E=<scratch> after-before.sh <pid-to-wait> <command...>
pid=$1; shift
while kill -0 "$pid" 2>/dev/null; do sleep 20; done
"$@"
