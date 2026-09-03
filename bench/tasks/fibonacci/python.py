#!/usr/bin/env python3
"""Iterative fibonacci — same algorithm as the handl/js/rust/c versions."""
import sys

n = int(sys.argv[1])
a, b = 0, 1
for _ in range(n):
    a, b = b, a + b
print(a)
