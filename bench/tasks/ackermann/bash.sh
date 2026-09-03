#!/usr/bin/env bash
# Recursive ackermann — same algorithm as the handl/js/python/rust/c versions.
# Usage: bash bash.sh 2 5
# Bash doesn't have functions that can recurse deep enough for ack(3,3),
# but we include it for completeness on the e2e args (2,5).
ack() {
    local m="$1" n="$2"
    if [ "$m" -eq 0 ]; then
        echo $((n + 1))
    elif [ "$n" -eq 0 ]; then
        ack $((m - 1)) 1
    else
        local inner
        inner=$(ack "$m" $((n - 1)))
        ack $((m - 1)) "$inner"
    fi
}
echo $(ack "$1" "$2")
