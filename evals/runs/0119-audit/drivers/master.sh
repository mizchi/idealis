#!/bin/bash
: "${E:?}"
W=$1
$E/drivers/slot-a.sh "$W"
$E/drivers/slot-b.sh "$W"
$E/drivers/slot-d.sh "$W" oracle-r2 4000 1400
$E/drivers/slot-d.sh "$W" after 600 300
$E/drivers/slot-c.sh "$W"
$E/drivers/slot-e.sh "$W"
echo done > $E/logs/master.done
