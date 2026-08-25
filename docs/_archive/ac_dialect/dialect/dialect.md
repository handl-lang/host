**handl_dialect > dialect > dialect**

# Module: dialect::dialect

## Contents

**Traits**

- [`Dialect`](#dialect) - A dialect for parsing and evaluating handl expressions and documents.

---

## handl_dialect::dialect::dialect::Dialect

*Trait*

A dialect for parsing and evaluating handl expressions and documents.

The handl language specification defines one canonical syntax. This trait
abstracts the syntax decisions so the implementation can be structured
around a single canonical dialect, with the architecture allowing others
if needed.

`Dialect` is a facade that groups four sub-traits:

- [`Syntax`] — interpolation markers, path separator, call delimiters.
- [`ExprParser`] — call detection, argument splitting, literal parsing.
- [`DocumentVocabulary`] — reserved key names (`fields`, `expr`, etc.).
- [`SchemaVocabulary`] — schema language for field and function validation.

Consumers that only need one aspect can depend on the sub-trait directly.
The facade provides a convenient "give me the whole language configuration"
accessor for call sites that need multiple aspects.

**Methods:**

- `syntax`: The syntax tokens — interpolation markers, path separator, call delimiters.
- `parser`: The expression parser — call detection, argument splitting, literal parsing.
- `document_vocabulary`: The document vocabulary — reserved key names used in handl documents.
- `schema_vocabulary`: The schema vocabulary used for field and function validation.



