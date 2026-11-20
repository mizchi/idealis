#!/bin/bash
# usage: tlc.sh <run-dir (must not exist)> <java> <apalache.jar> <quint-dir>
set -u
out=$1; java=$2; jar=$3; q=$4
mkdir "$out" || exit 98
for prop in safety broken; do
  d=$out/tlc-$prop
  mkdir "$d"
  cp "$q/NondetTags-$prop.tla" "$d/NondetTags.tla"
  printf 'INIT q_init\nNEXT q_step\nINVARIANT q_inv\n' > "$d/NondetTags.cfg"
  ( cd "$d" && "$java" -XX:+UseParallelGC -cp "$jar" tlc2.TLC -nowarning -deadlock -workers 1 -metadir "$d/states" -config NondetTags.cfg NondetTags.tla > "$d/out" 2>&1; echo "exit=$?" > "$d/exit" )
  echo "$prop: $(cat $d/exit)"
done
shasum -a 256 "$jar" "$q"/NondetTags-safety.tla "$q"/NondetTags-broken.tla > "$out/inputs.sha256"
