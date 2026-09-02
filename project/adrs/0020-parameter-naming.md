# Parameter Naming Convention

## Context

handl functions accept positional arguments. When functions are declared
in `*.fn.yml` files (ADR-0019) or in document-level `functions` blocks,
each parameter has a name. This name is used:

- In the function body to reference the argument value.
- In typegen output to document the function signature.
- In error messages when argument binding fails.

The naming convention for parameters affects readability, tooling, and
cross-language interop. This ADR establishes a convention, not a language
rule — the language accepts any valid identifier as a parameter name.

## Decision

### Convention

- Parameter names use `snake_case`.
- Names are nouns, not verbs (`prefix` not `do_prefix`).
- Boolean parameters use `is_`, `has_`, or `should_` prefixes
  (`is_strict`, `has_default`, `should_retry`).
- No abbreviations (`config` not `cfg`, `instance` not `inst`).

### Reserved names

The following names are reserved for the language runtime and must not be
used as parameter names:

- `self` — reserved for future method-like dispatch.
- `host` — reserved for host reflection.
- `args` — reserved for variadic argument access.

### Default values

Parameters may declare default values. A parameter with a default is
optional; a parameter without a default is required. Optional parameters
must come after required parameters in the declaration.

```yaml
params:
  - name: prefix
    default: "id"
  - name: length
    default: 8
```

## Consequences

- Function signatures are consistent and readable across the ecosystem.
- Typegen output produces clean API documentation.
- Error messages are clear when argument binding fails.
- The convention is enforced by the linter (ADR-0015), not the language
  kernel.

## Status

Proposed
