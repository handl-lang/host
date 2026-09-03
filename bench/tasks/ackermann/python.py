#!/usr/bin/env python3
"""Recursive ackermann — same algorithm as the handl/js/rust/c versions."""
import sys

def ack(m, n):
    if m == 0:
        return n + 1
    if n == 0:
        return ack(m - 1, 1)
    return ack(m - 1, ack(m, n - 1))

m = int(sys.argv[1])
n = int(sys.argv[2])
print(ack(m, n))
