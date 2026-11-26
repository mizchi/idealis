Oracle (limit 4,096): largest accepted size per cell and the evaluation depth it reaches; the minimum over cells per host.

| host | minimum depth | cell | size | next failure |
| --- | ---: | --- | ---: | --- |
| node default | 1553 | update | 1500 | RangeError: Maximum call stack size exceeded |
| node --liftoff-only | 1697 | update | 1639 | RangeError: Maximum call stack size exceeded |
| node --no-liftoff | 1261 | update | 1217 | RangeError: Maximum call stack size exceeded |
| chromium main | 1598 | update | 1543 | RangeError: Maximum call stack size exceeded |
| chromium worker | 827 | update | 797 | RangeError: Maximum call stack size exceeded |

After (limit 256): largest accepted size per cell, compared with the size whose depth is 255.

| host | cells | largest = expected | typed refusal next | untyped failure |
| --- | ---: | ---: | ---: | ---: |
| node default | 15 | 15 | 15 | 0 |
| node --liftoff-only | 15 | 15 | 15 | 0 |
| node --no-liftoff | 15 | 15 | 15 | 0 |
| chromium main | 15 | 15 | 15 | 0 |
| chromium worker | 15 | 15 | 15 | 0 |

cells whose largest accepted size differs from the expected one: 0
