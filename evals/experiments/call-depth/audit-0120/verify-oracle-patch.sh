for p in "o1 v1 oracle" "o2 v2 oracle-after"; do
 set -- $p
 for f in $(grep '^diff' chk4/$1.patch | sed 's/.* a\/\([^ ]*\) .*/\1/'); do cmp chk4/$2/$f trees/$3/$f || echo DIFF $f; done
 echo checked $3
done
