#!/usr/bin/env bash
# Iterative fibonacci — same algorithm as the handl/js/python/rust/c versions.
# Usage: bash bash.sh 35
n="$1"
a=0
b=1
i=0
while [ "$i" -lt "$n" ]; do
    tmp=$((a + b))
    a=$b
    b=$tmp
    i=$((i + 1))
done
echo "$a"
