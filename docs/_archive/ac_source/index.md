# handl_source

# handl-source

Source location tracking for the handl language.

Provides [`SourceLocation`] (1-indexed line:column) and [`SourceMap`]
(maps dotted paths to locations). Both are pure data structures —
format-specific scanning lives in each frontend crate.

## Architectural rule

> **`handl-source` is format-agnostic.** Each frontend builds a
> `SourceMap` using its own format-specific knowledge (indentation,
> braces, sections, etc.).

## Modules

### [`handl_source`](handl_source.md)

*3 modules*

### [`location`](location.md)

*1 struct*

### [`scanner`](scanner.md)

*2 functions, 3 structs*

### [`sourcemap`](sourcemap.md)

*1 struct*

