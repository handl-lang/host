# RFC-0002: Formatter in handl (Refactor of the Bootstrap Formatter)

## Status

Low priority. Replaces the temporary bootstrap formatter of
[ADR-0086](../adrs/0086-bootstrap-formatter.md) with a handl implementation.
Subsumes the _self-hosting_ half of ADR-0014 (the canonical-style rules of
ADR-0014 §Canonical style that are still correct carry over unchanged).

## Summary

Rewrite the formatter in handl as a pure arrow `Source ~> Source`, composed
of independent, testable formatting rules. The Rust `plugin-ext-fmt` crate is
retired as the primary once this implementation is at feature parity and
passes the same
golden corpus.

## Detailed design

- **The formatter is a description.** `format = collect-tokens >>> apply-rules >>> render`,
  where each stage is a handl arrow. Handlers interpret it: the CLI runs it
  via the eval engine; an LSP runs it incrementally per-document.
- **Rules are first-class.** Each canonical-style rule (indentation,
  chain-breaking, blank-line policy…) is a separate handl module
  `rule-name : TokenStream ~> TokenStream`. Rules compose with `>>>`;
  disabled rules are simply omitted from the composition. No rule registry,
  no plugin machinery — composition _is_ the extension mechanism.
- **Language-generic core, syntax-specific rules** (the property ADR-0014
  wanted but implemented the hard way): the core pipeline is parameterized
  by a _language value_ (per ADR-0041), so other handl languages get a
  formatter by supplying their token kinds and rule set.
- **Idempotence as a law:** `format >>> format ≍ format`, stated with
  ADR-0075 `law` syntax in the formatter module and property-tested.
- **Config:** the hardcoded preferences of ADR-0086 become a record with
  defaults (`{ indent = 2, width = 100, ... }`), threaded as the formatter's
  input rather than read from a config file; a `hnl` config layer can supply
  overrides.

## Migration

1. Port golden corpus from `plugin-ext-fmt/tests/golden/` — acceptance is
   byte-identical output on the full corpus (stdlib + examples).
2. Wire `hnl fmt` to the handl formatter, keeping the Rust fallback until
   step 3.
3. Retire `crates/plugin-ext-fmt` as the primary (Rust version remains the
   fallback per ADR-0085 rule 6).

## Drawbacks

- Formatter latency matters on keystroke-sized inputs; requires the
  compiled (not tree-walk) eval path before the LSP can use it.

## Related

- [ADR-0086](../adrs/0086-bootstrap-formatter.md) — the bootstrap being
  replaced; ADR-0014 — origin of the canonical style; ADR-0041 — languages
  as first-class values; [RFC-0007](0007-cli-in-handl.md) — CLI integration
  contract.
