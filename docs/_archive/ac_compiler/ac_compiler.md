**handl_compiler**

# Module: handl_compiler

## Contents

**Modules**

- [`config`](#config) - Pipeline configuration — holds the pluggable components.
- [`diagnostic`](#diagnostic) - Structured diagnostics — replaces `String` errors for passes that
- [`driver`](#driver) - Front-end phase pipeline backed by [`driver_lang`].
- [`source`](#source) - Source input for a compilation pipeline.
- [`visitor`](#visitor) - Visitor and Folder traits for generic IR traversal.

---

## Module: config

Pipeline configuration — holds the pluggable components.

Each component is a trait object, so callers can swap implementations:

- `resolver`: how modules are loaded (`handl-module-resolver-file`, `handl-module-resolver-virtual`, `handl-module-resolver-http`, `handl-module-resolver-git`)
- `interpreter`: how schema fragments become types (`handl-typecheck-schema-json`, future custom)
- `backend`: how types are emitted (`handl-typegen-backend-json-schema`, future TypeScript, Cue)

Default configurations (`default_cli`, `typecheck_only`) are provided
by `handl-compiler`, which depends on the implementation crates.

Custom passes can read their own config via [`Config::extension`]
without modifying this struct.



## Module: diagnostic

Structured diagnostics — replaces `String` errors for passes that
need severity levels, source spans, and diagnostic codes.

Passes can emit diagnostics via [`Compilation::emit`](crate::Compilation::emit).
The pipeline can be configured to stop on errors or collect all.



## Module: driver

Front-end phase pipeline backed by [`driver_lang`].

Each compilation phase is a [`Stage<Config>`] with typed `Input` and
`Output` artifacts. The compiler enforces at compile time that phases
connect correctly — you cannot wire a generate stage after a parse
stage because the output type of parse does not match the input type
of generate.

## Artifact flow

```text
Source → ParsedDoc → ExtractedDoc → ResolvedDoc → CheckedDoc → JsonValue
```

## Preset pipelines

- **typegen**: `Source → ParsedDoc → ExtractedDoc → ResolvedDoc → JsonValue`
- **typecheck**: `Source → ParsedDoc → ExtractedDoc → TypecheckResult`
- **full**: `Source → ParsedDoc → ExtractedDoc → ResolvedDoc → CheckedDoc → JsonValue`

## Stage implementations

Stage impls live in the pass crates:

- `handl-compiler-pass-parse` — `ParseStage`
- `handl-compiler-pass-extract` — `ExtractStage`
- `handl-compiler-pass-resolve` — `ResolveStage`
- `handl-compiler-pass-typecheck` — `TypecheckStage`, `TypecheckOnlyStage`
- `handl-compiler-pass-generate` — `GenerateStage`, `GenerateFromResolvedStage`

## Custom stages

```ignore
use driver_lang::{Pipeline, Session, Stage};
use handl_compiler::driver::*;

struct MyLintStage;
impl Stage<Config> for MyLintStage {
    type Input = ExtractedDoc;
    type Output = ExtractedDoc;
    fn name(&self) -> &'static str { "lint" }
    fn run(&mut self, input: ExtractedDoc, session: &mut Session<Config>)
        -> Result<ExtractedDoc, DriverError>
    {
        // inspect input.extracted, emit warnings via session
        Ok(input) // pass through
    }
}

let pipeline = Pipeline::new(ParseStage)
    .then(ExtractStage)
    .then(MyLintStage)
    .then(ResolveStage)
    .then(GenerateStage);
```



## Module: source

Source input for a compilation pipeline.



## Module: visitor

Visitor and Folder traits for generic IR traversal.

`Visitor` walks an IR tree without mutating — analysis passes
(lint, metrics) implement `Visitor`.

`Folder` transforms an IR tree, returning a new one — optimization
passes implement `Folder`.

Both have default implementations that recurse, so passes override
only the nodes they care about.



