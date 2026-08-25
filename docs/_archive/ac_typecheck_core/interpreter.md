**handl_typecheck_core > interpreter**

# Module: interpreter

## Contents

**Traits**

- [`SchemaInterpreter`](#schemainterpreter) - Interprets schema fragments as types.

---

## handl_typecheck_core::interpreter::SchemaInterpreter

*Trait*

Interprets schema fragments as types.

Each schema language (JSON Schema, Cue, custom) implements this trait
to convert its schema vocabulary into the neutral [`Type`] enum.
The typechecker uses the interpreter to build [`TypeEnv`]s from
raw field schemas.

Implementations:
- `handl-typecheck-schema-json` — JSON Schema → Type
- Future: Cue, custom schema languages

**Methods:**

- `interpret`: Interpret a raw schema fragment as a [`Type`].



