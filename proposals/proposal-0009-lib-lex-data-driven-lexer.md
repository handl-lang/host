+++
id = "P-009"
title = "lib-lex data-driven lexer"
status = "open"
created = "2026-11-23"
priority = "low"
labels = ["lex", "parsing"]
+++

# P-009 — lib-lex data-driven lexer

***Status:** draft · **Priority:** low · **New repos:** `lib-lex` (contract),
`lib-lex-pattern` (backend) · **Task:** T-139
**Port of:** `tool-lex` + `tool-lex-pattern` (Rust, ADR-0044 §3)
**Unblocks:** app-level scanning — P-008 `cli-router` `tokens`,
T-141 lib-repl line tokenizing, msg-router payload decoding, any
handl-side DSL/config scanning. NOT the hnl language lexer (P-010
uses `tool-parse`, which stays Rust-side).

## Idea — port/impl split (org naming law)

Same split as `tool-lex`/`tool-lex-pattern`, expressed as
contract/backend:

- **`lib-lex`** — the *port*: `Token`/`Tokens` record shapes, the
  `lex(rules, src, opts?)` signature + doc, the error contract
  (`{pos, msg}` raise or `opts.on_error`), whitespace/bracket-pair
  conventions. No implementation — backends export arrows of these
  types (the `stdio`/`fetch` discipline).
- **`lib-lex-pattern`** — the reference backend: the data-driven rule
  engine. `LexerRuleDef` fragments become plain records
  (`{id, kind, pat, pri}`) where `pat` is one of Literal / CharClass /
  Number / Delimited — rules in, token stream out, one pure function.

```handl
from "lex_pattern" import { lex, shell_words }   -- backend entry
from "lex" import { Token }                      -- contract types

rules := [
  { id: "num",  kind: "num",  pat: { t: "number" },              pri: 1 },
  { id: "id",   kind: "id",   pat: { t: "class", start: "a-zA-Z_",
                                             cont: "a-zA-Z0-9_" }, pri: 2 },
  { id: "str",  kind: "str",  pat: { t: "delimited", open: "\"",
        close: "\"", escape: true },                            pri: 3 },
  { id: "plus", kind: "op",   pat: { t: "lit", text: "+" },      pri: 4 },
]

toks := lex(rules, "foo + 42")   -- [{kind:"id",text:"foo",...}, ...]
words := shell_words("set name \"John Doe\"")  -- quote-aware preset
```

## Pattern vocabulary (mirrors `LexerPattern`)

| Rust variant | `.hnl` record | Notes |
|---|---|---|
| `Literal{text}` | `{t:"lit", text}` | longest-match tiebreak within priority |
| `CharClass{start,cont}` | `{t:"class", start, cont}` | `a-z` range syntax parsed once at compile |
| `Number` | `{t:"number"}` | optional `-`, digits, `.`+digits |
| `Delimited{open,close,escape,interpolation,nesting_open,close_at_eof}` | `{t:"delimited", ...}` | escape `\`, `${}` interp flag, brace-nesting depth |

`compile(rules)` pre-sorts by `pri` asc + literal length desc and
pre-parses charclass ranges → matchers are then pure record checks.

## Engine semantics (ported verbatim)

- skip whitespace between tokens;
- first matching rule in priority order wins; no match → `error.raise`
  with position (or `opts.on_error` for lenient modes);
- bracket pairing: rules may declare `{pair: {open: kind, close: kind}}`
  — a stack enforces nesting (simpler than the Rust token-category
  lookup, same observable behavior);
- `literal_kinds` boundary rule from the Rust port carries over:
  value-literal kinds skip the `:`/`.` name-position boundary check.

## Token shape

`{ kind, text, start, end }` — `text` is the raw slice; value
coercion (string unquoting, number parsing) is the *consumer's* job
or an opt-in `opts.coerce` map, keeping the lexer honest.

## Presets (in `lib-lex-pattern`)

- `shell_words` — quote/escape-aware splitting for REPL lines and
  `sh -c`-style parsing (the P-008 `tokens` need);
- maybe `json`/`hnl`-lite rulesets later — the handl grammar itself
  stays in `tool-lex-pattern` + YAML fragments (Rust side keeps its
  job; this lib is for *application* lexing, not the language).

## Non-goals

- `lib-lex` ships no matcher — alternate backends (regex-ish, hand-rolled
  per-language) can implement the same contract later.
- Not a replacement for `tool-lex-pattern` — that stays the
  language-definition lexer driven by `Language` fragments. `lib-lex`
  is the app-level scanner (rules as plain data, no fragment
  machinery).
- No parser — tokens only. `lib-parse` territory after that.
- No regex — the 4-variant pattern vocabulary is deliberately small.

## Tests

Port `tool-lex-pattern/tests/integration.rs` cases to `.hnl`;
charclass range edge cases; delimited nesting/escape/interp;
priority + longest-literal ordering; error position reporting;
`shell_words` quote/escape table.
