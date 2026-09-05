# Comparison: Arrow Support vs Arrow-Native

## Haskell

Haskell is **arrow-capable, not arrow-native**.

It has:

```text
Arrow
├── arr
├── >>>
├── first
├── ArrowChoice
├── ArrowLoop
└── ArrowApply
```

and both ordinary functions `(->)` and `Kleisli` arrows are instances.
`ArrowApply` is particularly interesting because Haskell documents it as
equivalent in expressive power to monads.

But Haskell's actual foundation remains:

```text
λ-calculus
  +
type system
  +
functions
  +
monads/etc.
  +
arrows as an abstraction
```

Arrow syntax is an embedded language, not the language kernel.

## Other languages

There are languages and DSLs heavily based on arrows — particularly **FRP,
hardware description, signal-processing, parser, and dataflow DSLs** — but
there is no mainstream general-purpose language whose _core computational
calculus_ is Arrow Calculus in the way this project proposes.

That is an important distinction. There is a huge difference between:

```text
Language supports arrows
```

and:

```text
Language IS an arrow calculus
```

This project is the second.

## The comparison

|                 | Haskell                        | This proposal                              |
| --------------- | ------------------------------ | ------------------------------------------ |
| Functions       | fundamental                    | derived/pure arrows                        |
| Lambda calculus | foundation                     | potentially unnecessary                    |
| Arrows          | library/typeclass abstraction  | **kernel**                                 |
| `>>>`           | arrow combinator               | fundamental composition                    |
| `choice`        | ArrowChoice                    | kernel                                     |
| `loop`          | ArrowLoop                      | kernel                                     |
| `app`           | ArrowApply                     | kernel if higher-order arrows needed       |
| Effects         | primarily monadic, also arrows | arrow operations + handlers                |
| Programs        | ordinary values/terms          | **first-class computational descriptions** |
| Language        | fixed language                 | **first-class `Language` value**           |
| Tooling         | external ecosystem             | generic protocol over `Language`           |

## The research gap

The interesting research gap is therefore not _"Haskell has arrows, so why
another language?"_

It is:

> **What happens if we take what Haskell treats as an embedded Arrow
> abstraction and make it the foundational computational calculus of the
> language — and then make the language itself first-class?**

That is a much stronger and, as far as this survey goes, substantially less
explored design point.
