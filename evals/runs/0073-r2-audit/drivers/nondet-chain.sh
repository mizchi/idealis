#!/bin/bash
E=<scratch>/eval-0073-r2
$E/drivers/timed.sh 0073-r2-nondet-native-rss <after> node $E/drivers/nondet-native-rss.mjs time $E/runs/0073-r2-nondet-native-rss $E/snapshots/before $E/snapshots/after <scratch>/quint || exit 1
$E/drivers/timed.sh nondet-native-2-capped <after> node $E/drivers/nondet-native-rss.mjs capped $E/runs/nondet-native-2-capped $E/snapshots/before $E/snapshots/after <scratch>/quint <rss-cap>/capped.py || exit 1
