---
title: 'ADR-0001 (Remaining): Canonical Token/Grammar/Value Plugin Cleanup'
type: adr
date: 2026-09-02
status: proposed
---

# ADR-0001 (Remaining): Canonical Token/Grammar/Value Plugin Cleanup

## Status

Proposed

## Context

ADR-0001 (Generic Node, Trait-Based Capabilities, and Plugin-Driven Canonical Kit) has been implemented and archived. The core generic `Node` type, the `Token`/`Lexer` and `Parser`/`Grammar` abstractions, and the `kit-canonical` plugin manifest are in place. The old `handl-ast-canonical`, `handl-lexer-canonical`, and `handl-parser-canonical` crates have been removed.

What remains is the cleanup and stabilization of the remaining canonical token, grammar, and value plugins.

## Decision

Finish cleanup and stabilization of the remaining `plugin-token-*`, `plugin-grammar-*`, and `plugin-value-*` canonical plugins so that `kit-canonical` is a complete, auditable plugin manifest with no language-specific code in the base framework.

## Consequences

- All canonical token, grammar, and value plugins become pure, composable, and testable plugins.
- `kit-canonical` can be finalized as a curated manifest.
- Once this is complete, no canonical-language implementation work remains in the base framework.

## Related work

- `project/adrs/archived/0001-generic-node-and-plugin-driven-canonical-kit.md`
