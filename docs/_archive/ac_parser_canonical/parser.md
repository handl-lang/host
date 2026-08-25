**handl_parser_canonical > parser**

# Module: parser

## Contents

**Structs**

- [`CanonicalGrammar`](#canonicalgrammar)
- [`CanonicalParser`](#canonicalparser) - The canonical handl expression parser.

**Enums**

- [`MatchPatternShape`](#matchpatternshape)

**Functions**

- [`to_ac_parse_error`](#to_ac_parse_error)

**Statics**

- [`PLACEHOLDER_COUNTER`](#placeholder_counter)

---

## handl_parser_canonical::parser::CanonicalGrammar

*Struct*

**Unit Struct**

**Methods:**

- `fn has_top_level_assignment(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> bool` - Returns `true` if there is a top-level `=` in the remaining token
- `fn parse_postfix(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_expr(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_expr_bp(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>, min_bp: u32) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn infix_binding(self: &Self, kind: &CanonicalKind) -> Option<(u32, u32)>`
- `fn parse_primary(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_kind(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<String, DefaultParseError>`
- `fn find_matching_call_close(self: &Self, context: &ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>, start: usize) -> Option<usize>`
- `fn parse_parenthesized(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_call_args(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Vec<Expr<NonFunctionValue>>, DefaultParseError>`
- `fn parse_after_identifier(self: &Self, name: String, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_call_or_named_function(self: &Self, name: String, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_function_literal(self: &Self, type_params: Vec<String>, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_function_parts(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<(Vec<(Pattern, Option<Box<Expr<NonFunctionValue>>>)>, Option<Box<Expr<NonFunctionValue>>>, String), DefaultParseError>`
- `fn parse_param_list(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Vec<(Pattern, Option<Box<Expr<NonFunctionValue>>>)>, DefaultParseError>`
- `fn parse_pattern(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Pattern, DefaultParseError>`
- `fn parse_array_pattern(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Pattern, DefaultParseError>`
- `fn parse_object_pattern(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Pattern, DefaultParseError>`
- `fn parse_object_pattern_entry(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<(String, Pattern), DefaultParseError>`
- `fn parse_generic_params(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Vec<String>, DefaultParseError>`
- `fn capture_body(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<String, DefaultParseError>`
- `fn parse_array(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_object(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_object_entry(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<(String, Expr<NonFunctionValue>), DefaultParseError>`
- `fn parse_handle(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_handler_clause(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<HandlerClause<NonFunctionValue>, DefaultParseError>`
- `fn parse_match(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn parse_match_pattern(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<(NonFunctionValue, Vec<String>), DefaultParseError>`
- `fn parse_match_pattern_core(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<(MatchPatternShape, Vec<String>), DefaultParseError>`
- `fn parse_match_array_pattern(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<(MatchPatternShape, Vec<String>), DefaultParseError>`
- `fn parse_match_object_pattern(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<(MatchPatternShape, Vec<String>), DefaultParseError>`
- `fn parse_match_object_pattern_entry(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<(String, NonFunctionValue, Vec<String>), DefaultParseError>`
- `fn shape_to_json(self: &Self, shape: MatchPatternShape, bindings: & mut Vec<String>) -> NonFunctionValue`
- `fn make_partial(self: &Self, expr: Expr<NonFunctionValue>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- `fn has_placeholder(self: &Self, expr: &Expr<NonFunctionValue>) -> bool`
- `fn rename_placeholders(self: &Self, expr: & mut Expr<NonFunctionValue>, params: & mut Vec<String>, nonce: usize)`
- `fn expr_to_canonical(self: &Self, expr: &Expr<NonFunctionValue>) -> Result<String, DefaultParseError>`
- `fn pattern_to_canonical(self: &Self, pattern: &Pattern) -> Result<String, DefaultParseError>`

**Traits:** Copy

**Trait Implementations:**

- **Grammar**
  - `fn rule_for(self: &Self, _kind: &CanonicalKind) -> Option<handl_parser_common::Rule<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>>`
  - `fn eof(self: &Self) -> Option<CanonicalKind>`
  - `fn parse_root(self: &Self, context: & mut ParserContext<CanonicalKind, Expr<NonFunctionValue>, DefaultParseError>) -> Result<Expr<NonFunctionValue>, DefaultParseError>`
- **Default**
  - `fn default() -> CanonicalGrammar`
- **Clone**
  - `fn clone(self: &Self) -> CanonicalGrammar`



## handl_parser_canonical::parser::CanonicalParser

*Struct*

The canonical handl expression parser.

Recursive-descent parser that consumes a flat token list and
builds an [`Expr`] tree. Stateless — safe to reuse across inputs.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Parser**
  - `fn parse(self: &Self, tokens: Vec<Token<CanonicalKind>>) -> Result<Expr<NonFunctionValue>, HandlParseError>`
  - `fn parse_text(self: &Self, input: &str, lexer: &dyn handl_lexer::Lexer) -> Result<Expr<NonFunctionValue>, HandlParseError>`
- **Clone**
  - `fn clone(self: &Self) -> CanonicalParser`
- **Default**
  - `fn default() -> CanonicalParser`



## handl_parser_canonical::parser::MatchPatternShape

*Enum*

**Variants:**
- `Json(ac_runtime_values::NonFunctionValue)`
- `Binding(String)`
- `Wildcard`



## handl_parser_canonical::parser::PLACEHOLDER_COUNTER

*Static*

```rust
static PLACEHOLDER_COUNTER: std::sync::atomic::AtomicUsize
```



## handl_parser_canonical::parser::to_ac_parse_error

*Function*

```rust
fn to_ac_parse_error(e: handl_parser_common::DefaultParseError) -> handl_parser::ParseError
```



