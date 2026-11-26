Oracle-after (limit 4,096, 256 compared with the evaluation depth): largest accepted size per cell and the 256-compared depth it reaches; the minimum over the cells whose next failure is a host failure.

| host | minimum depth | cell | size | next failure |
| --- | ---: | --- | ---: | --- |
| node default | 1295 | update | 1250 | RangeError: Maximum call stack size exceeded |
|   (node default: bounded by the 4,096 limit first) | 4092 | function | 1006 | error protocol: Safety evaluation nesting exceeds 256 peak=4 |
| node --liftoff-only | 1311 | operator | 1267 | RuntimeError: memory access out of bounds |
|   (node --liftoff-only: bounded by the 4,096 limit first) | 4095 | membership | 3960 | error protocol: Safety evaluation nesting exceeds 256 peak=4 |
|   (node --liftoff-only: bounded by the 4,096 limit first) | 4092 | function | 1006 | error protocol: Safety evaluation nesting exceeds 256 peak=4 |
| node --no-liftoff | 1165 | update | 1124 | RangeError: Maximum call stack size exceeded |
|   (node --no-liftoff: bounded by the 4,096 limit first) | 4092 | function | 1006 | error protocol: Safety evaluation nesting exceeds 256 peak=4 |
| chromium main | 1311 | operator | 1267 | RuntimeError: memory access out of bounds |
|   (chromium main: bounded by the 4,096 limit first) | 4095 | membership | 3960 | error protocol: Safety evaluation nesting exceeds 256 peak=4 |
|   (chromium main: bounded by the 4,096 limit first) | 4093 | lazy | 1980 | error protocol: Safety evaluation nesting exceeds 256 peak=4 |
|   (chromium main: bounded by the 4,096 limit first) | 4092 | function | 1006 | error protocol: Safety evaluation nesting exceeds 256 peak=4 |
| chromium worker | 768 | update | 740 | RangeError: Maximum call stack size exceeded |
|   (chromium worker: bounded by the 4,096 limit first) | 4092 | function | 1006 | error protocol: Safety evaluation nesting exceeds 256 peak=4 |

After (limit 256): largest accepted size per cell, compared with the size whose 256-compared depth is 255 (boundaries-255.json).

| host | cells | largest = expected | typed refusal next | untyped failure |
| --- | ---: | ---: | ---: | ---: |
| node default | 19 | 19 | 19 | 0 |
| node --liftoff-only | 19 | 19 | 19 | 0 |
| node --no-liftoff | 19 | 19 | 19 | 0 |
| chromium main | 19 | 19 | 19 | 0 |
| chromium worker | 19 | 19 | 19 | 0 |

oracle-after hosts whose minimum depth is below 512 (required 0): 0
cells whose largest accepted size differs from the expected one, or other problems: 0
