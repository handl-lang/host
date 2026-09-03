# ackermann

Recursive Ackermann — the classic call-stack / function-dispatch stress test.

## Algorithm (identical in all languages)

```
ack(m, n):
    if m == 0: return n + 1
    if n == 0: return ack(m - 1, 1)
    return ack(m - 1, ack(m, n - 1))
```

## Parameters

| Mode         | m   | n   | Expected output | handl ~time |
| ------------ | --- | --- | --------------- | ----------- |
| end-to-end   | 2   | 5   | 13              | ~0.9 s      |
| heavy-work   | 3   | 3   | 61              | ~20 s       |

## Why this task

- Pure recursion with nested recursive calls — stress-tests function dispatch,
  closure invocation, and call-stack management.
- handl uses a self-applying closure (`self ~> self(self, m, n)`) since the
  arrow-calculus kernel has no named recursion primitive; recursion is
  achieved via explicit self-passing (ADR-0052).
- This is where interpreted languages diverge most from compiled ones.
  The gap **is the data** — it shows the cost of handl's tree-walk evaluator
  vs JIT (bun/node) vs AOT (rust/c).
