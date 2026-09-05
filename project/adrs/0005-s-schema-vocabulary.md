# handl-native `s.*` Schema Vocabulary

## Context

handl's canonical schema vocabulary is currently JSON Schema draft 2020-12. It is a separate, external language embedded in handl documents. Runtime validation, type inference, and type generation are all coupled to JSON Schema keywords (`type`, `properties`, `items`, `allOf`, etc.). This creates the friction that motivated this redesign: the language already has a flexible expression syntax and a generic `Schema`/`SchemaVocabulary` trait, yet the canonical path forces authors to learn and write JSON Schema.

The `Schema`/`SchemaVocabulary` traits and `SchemaInterpreter` trait already provide the seams for a different schema language. Object and array literals are being added to the handl expression grammar (ADR-0003). With that in place, handl can express rich schemas in its own syntax.

This ADR is heavily inspired by [Valibot](https://valibot.dev/): a small, tree-shakeable, pipe-based schema library. Valibot's core insight is to separate **base types** (`string()`, `number()`, `object()`), **collection combinators** (`array()`, `object()`, `union()`, `optional()`), and **actions/constraints** (`minLength()`, `range()`, `email()`). Schemas are composed by piping a base type through one or more actions.

## Decision

Replace JSON Schema with an handl-native `s.*` vocabulary as the canonical schema language. The `s` namespace is a set of compiler-known schema constructors and actions, not a runtime `Host` namespace. Schemas are source-code descriptions that the compiler turns into `Schema` validators, `Type` objects, and typegen output.

### Document shape

Add `schema` to the canonical document vocabulary as the field-level key that holds the schema expression. A field may have `schema`, `expr`, `arg`, `merge`, `inheritable`, and `mutable` together.

```yaml
$:
  name:
    schema: s.pipe(s.string(), s.min_length(1))
  port:
    schema: s.pipe(s.integer(), s.range(1, 65535))
  tags:
    schema: s.array(s.string())
  config:
    schema: s.object({
      host: s.string(),
      port: s.integer()
    })
  status:
    schema: s.pick_list("active", "inactive", "pending")
```

Function schemas use the same `s` vocabulary under `params` and `return`:

```yaml
$:
  add:
    params:
      - name: a
        schema: s.number()
      - name: b
        schema: s.number()
    return: s.number()
    body: add(a, b)
```

### The `s` namespace

`s` is a reserved namespace recognized by the schema compiler. Its forms are not evaluated by the VM; they are parsed and compiled by the `SchemaVocabulary`, `SchemaInterpreter`, and `TypegenBackend`.

#### Base types

```
s.string()
s.number()
s.integer()
s.boolean()
s.null()
s.unknown()
s.any()
s.never()
```

#### Literals and unions

```
s.literal("admin")
s.pick_list("admin", "user", "guest")
s.union(s.string(), s.number())
```

#### Collections

```
s.array(s.string())
s.object({ host: s.string(), port: s.integer() })
```

#### Optionality

```
s.optional(s.string())    # null or string
s.nullable(s.string())    # alias for optional in handl; null or string
```

`optional`/`nullable` are initially typed as `Unknown` because the `Type` enum does not yet have union or option types. This is a known, explicit limitation. When the type system gains unions, these forms will be upgraded.

#### Constraints via `s.pipe`

The central composition operator is `s.pipe(base, ...actions)`. The first argument is any `s` form. Remaining arguments are actions that refine the set of valid values without changing the base type.

```
s.pipe(s.string(), s.min_length(1))
s.pipe(s.string(), s.min_length(1), s.max_length(100))
s.pipe(s.integer(), s.range(1, 65535))
s.pipe(s.number(), s.min_value(0))
s.pipe(s.string(), s.regex("^[a-z]+$"))
```

This separation makes the typechecker's job simple: the type of a `s.pipe` expression is the type of its first argument. Constraints affect validation and code generation, not static type inference.

#### Initial action set

The first version supports a minimal, extensible set of actions:

- Strings: `s.min_length(n)`, `s.max_length(n)`, `s.length(n)`, `s.regex(pattern)`
- Numbers/Integers: `s.min_value(n)`, `s.max_value(n)`, `s.range(min, max)`, `s.multiple_of(n)`
- Arrays: `s.min_items(n)`, `s.max_items(n)`, `s.length(n)`
- Generic: `s.brand(name)` (for branded types; typegen can preserve the name as metadata)

More actions can be added without changing the overall design.

### Architecture changes

1. **`handl-dialect-canonical`** provides `SSchemaVocabulary` and `SSchema` as the canonical implementations of `SchemaVocabulary<NonFunctionValue>` and `Schema<NonFunctionValue>`. `SSchema` parses and evaluates the `s.*` expression tree against values.
2. **`handl-typecheck-schema-s`** (new crate) provides `SSchemaInterpreter`, implementing `SchemaInterpreter` by mapping `s.*` forms to `Type`.
3. **`handl-typegen-backend-s`** (new crate) provides an `SBackend` that emits the canonical `s` schema as a JSON-serializable value (a string or structured form). It is the default output for `typegen`.
4. **`handl-typegen-backend-json-schema`** is rewritten as a translator from `s` to JSON Schema. It becomes an optional backend for external tooling, not the canonical format.
5. **`handl-fields`** removes the JSON-Schema-specific `check_arg_type` in `computed.rs` and instead compiles `arg` schemas through `dialect.schema_vocabulary()`.
6. **`handl-compiler`** default configs use `SSchemaInterpreter` and `SBackend` instead of the JSON Schema implementations.

### Type inference rules

The `SchemaInterpreter` handles the most common forms directly:

| `s` form                          | `Type`                                      |
| --------------------------------- | ------------------------------------------- |
| `s.string()`                      | `Type::String`                              |
| `s.number()`                      | `Type::Number`                              |
| `s.integer()`                     | `Type::Number` or a future `Type::Integer`  |
| `s.boolean()`                     | `Type::Boolean`                             |
| `s.null()`                        | `Type::Null`                                |
| `s.array(inner)`                  | `Type::Array(Box::new(inner_type))`         |
| `s.object({ k: v, ... })`         | `Type::Object(BTreeMap { k: v_type, ... })` |
| `s.pipe(base, ...)`               | `type_of(base)`                             |
| `s.optional(a)` / `s.nullable(a)` | `Type::Unknown` until unions exist          |
| `s.union(...)`                    | `Type::Unknown` until unions exist          |

This is intentionally conservative. The `Type` enum is not being expanded in this ADR. Losing precision for `optional` and `union` is an acceptable cost for replacing JSON Schema as the canonical vocabulary.

### Typegen output

The canonical `TypegenBackend` outputs `s` schemas. The JSON Schema backend remains available as a translator. This keeps the canonical language self-contained while preserving the ability to produce JSON Schema for external consumers.

## Consequences

**Positive:**

- The canonical schema language is now handl-native. Authors write one syntax for values and types.
- The `pipe` model from Valibot is small, composable, and easy to extend with new actions without touching the core language.
- The `Schema`/`SchemaVocabulary`/`SchemaInterpreter` traits finally have a canonical implementation that is not JSON Schema, validating the architecture.
- The `TypegenBackend` becomes a translator, which is cleaner than emitting JSON Schema directly from field data.
- JSON Schema compatibility is retained as an optional backend rather than the default.

**Negative:**

- This is a breaking change. Existing documents using `type:`, `properties:`, `items:`, and other JSON Schema keywords will need to migrate to `s.*` forms.
- `s` forms are not first-class runtime values. They are special forms recognized by the schema compiler. This creates a "shadow language" inside handl expressions, even though it reuses the same syntax.
- `s.optional()` and `s.union()` lose type precision until the `Type` enum supports unions/option types.
- The `ExprParser` and `eval_arg` changes from ADR-0003 must be complete and stable before this work can land.
- More crates to maintain: `handl-typecheck-schema-s`, `handl-typegen-backend-s`, and the JSON Schema translator backend.

**Open questions:**

- Should `s.integer()` produce a distinct `Type::Integer` or remain `Type::Number`? The existing `check_arg_type` special-cases "integer"; a proper `Type::Integer` may be cleaner.
- Should `s.object` support `additionalProperties` / `record` semantics, or only closed records?
- Should `s.pick_list` be typed as the literal union of its values, or as the underlying primitive (`Type::String`)?
- How are recursive/self-referential schemas expressed (`s.ref("name")` or `s.lazy(...)`)?
- Should the `.handl` frontend support inline schema shorthands (e.g. `name: s.string` without `schema:`)?

**Non-goals:**

- This ADR does not add arbitrary schema-as-predicate support. Full handl boolean predicates as schemas are intentionally out of scope because the typechecker cannot derive a static `Type` from an arbitrary validator.
- It does not remove the `Schema`/`SchemaVocabulary` abstraction. Custom schema languages remain possible.
- It does not define a runtime `Host` namespace for `s`. `s` is a compile-time namespace.

## Status

Proposed
