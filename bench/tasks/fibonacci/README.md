# fibonacci

Iterative Fibonacci — tight arithmetic loop with a counter and two accumulators.

## Algorithm (identical in all languages)

```
a, b = 0, 1
repeat n times:
    a, b = b, a + b
return a
```

## Parameters

| Mode         | n  | Expected output |
| ------------ | -- | --------------- |
| end-to-end   | 35 | 9227465         |
| heavy-work   | 38 | 39088169        |

## Why this task

- Tests tight-loop integer arithmetic and variable rebinding.
- Iterative, so call-stack overhead is not a factor — pure loop speed.
- handl uses `while` feedback (state flows through `{ state: ... }` / `return`),
  which is the idiomatic arrow-calculus loop pattern (ADR-0074).
