# ADR-0080: `handl-press` — A VitePress-Class Docs Framework Written In and Configurable Through Handl

## Status

Proposed. Targets the backlog (see `project/BACKLOG.md` → "Low priority").
Depends on [ADR-0077 — Stdlib Migration to Handl](./0077-stdlib-migration-to-handl.md)
(`op`/`handl`/`cap` surface, `HandlModule` loader), [ADR-0069 — Arrow-Native
Modules](./0069-arrow-native-modules.md) (importable `.hnl` modules),
[ADR-0021 — HMR](./0021-hmr.md) (dev-server hot reload), and
[ADR-0079 — Unified Dialect](./0079-unify-dialects-no-let-in.md) (graph blocks
as the configuration surface).

## Context

### What VitePress is and why it is the reference

VitePress is the canonical "docs framework" shape in the frontend ecosystem:
Markdown content + a small JS/TS config + Vue components for extensibility,
compiled by Vite to a static site (SSG) with a dev server (HMR), client-side
routing, theming, search, and i18n. Its success comes from three properties:

1. **Content is plain Markdown** — authoring stays low-friction.
2. **Configuration is code** — `config.ts` is a real program, not a data file.
   Loops, conditionals, and imports generate nav, sidebar, and routes.
3. **Components are first-class** — any `.vue` file is a slot the content can
   pull in, so customization does not require forking the framework.

The third property is the one static-site generators that predate VitePress
(Jekyll, Hugo, MkDocs) approximate with templating languages but never reach
fully, because their config and templates are not the same language as the
runtime. VitePress closes that loop by being built _in_ the language it
configures.

### Why handl is a strictly better fit than JS/TS for this

handl's design (per `project/principles.md` and the ADR chain through 0077)
gives it three properties JS/TS does not have, all of which matter for a docs
framework:

1. **Configuration is a typed, capability-tracked program.** A handl config
   file is not just "a script that runs" — it is an arrow graph whose
   `cap` declarations make its I/O surface explicit (ADR-0077 Phase 5). A
   theme that only reads `fs.read` cannot accidentally exfiltrate content over
   `http`. This is the property VitePress approximates with bundler sandboxing
   and that handl gets from the language.

2. **The same language authors content, config, themes, and the build
   pipeline.** Markdown is the content layer; everything _around_ it — nav
   generation, sidebar grouping, route tables, theme rendering, asset
   pipelines, search indexing — is handl. There is no second language to learn
   and no boundary crossing between "config language" and "template
   language."

3. **Multi-target output is inherent.** ADR-0077's interface/impl split
   (`op` declared in `.hnl`, `handl` bound per target) means the same docs
   program runs as a dev server (`http-server-std`), builds to static files
   (`fs` writes), renders SSR (`http-server` + in-process eval), or compiles
   to WASM for in-browser rendering — by swapping impl crates, not by
   rewriting the framework. VitePress needs a separate Vite SSR pipeline to
   achieve this.

### The gap

There is no docs framework in the handl ecosystem. The platform has the
primitives — `plugin-lib-markdown` does not exist yet but `plugin-lib-yaml`,
`plugin-lib-json`, `plugin-lib-toml`, `plugin-lib-fs`, `plugin-lib-http-server`,
`plugin-lib-http-client`, `plugin-lib-unicode`, `plugin-lib-intl`,
`plugin-lib-dedent`, `plugin-lib-utils`, `plugin-lib-collections`,
`plugin-lib-tag` (HTML emission candidate), and the `kit-server` /
`kit-config` kits already exist. What is missing is the _assembly_: a kit
that curates these into a VitePress-class framework with a defined content
format, config schema, theme protocol, and build pipeline.

### Why this is low priority

The platform's stated priority is the language runtime and stdlib migration
(ADR-0077). A docs framework is a _consumer_ of the stable end-state of that
work, not a contributor to it. Building it before ADR-0077 Phase 4 (native
lib migration) and Phase 5 (capability declaration) lands would mean
re-implementing capability tracking and `op` dispatch in the framework
itself, which is exactly the per-library Rust scaffolding ADR-0077 is
eliminating. This ADR is recorded now so the design is fixed and the
backlog entry is concrete; implementation waits on the dependencies.

## Decision

Build `handl-press`: a VitePress-class docs framework authored in and
configurable through handl. It is structured as one kit (`kit-press`) curating
a small set of `plugin-lib-*` libraries plus a content/theme protocol defined
in `.hnl`. No new runtime mechanism is invented; the framework is an
_application_ of ADR-0077's end state.

### 1. Layering — kit, libs, protocol

```
kit-press                       (curated plugin list, ADR-0039)
  ├─ plugin-lib-markdown        (new — block parser, handl-in-md fences)
  ├─ plugin-lib-docs-config     (new — config schema, validation, defaults)
  ├─ plugin-lib-docs-theme      (new — theme protocol, slot rendering)
  ├─ plugin-lib-docs-build      (new — SSG pipeline, asset graph, manifest)
  ├─ plugin-lib-docs-search     (new — index build + client query, optional)
  ├─ plugin-lib-tag             (existing — HTML emission)
  ├─ plugin-lib-fs              (existing — read content, write dist)
  ├─ plugin-lib-http-server     (existing — dev server)
  ├─ plugin-lib-http-server-std (existing — dev server impl)
  ├─ plugin-lib-intl            (existing — i18n)
  ├─ plugin-lib-utils           (existing)
  └─ plugin-lib-collections     (existing)
```

The four new `plugin-lib-docs-*` libs follow ADR-0077: interface in `.hnl`
(`op` + `cap` + axioms + `---` doc), impl in Rust only where irreducible
(Markdown block parser, full-text index). Logic — nav generation, sidebar
grouping, route tables, manifest assembly, theme composition — is handl.

### 2. Content format — Markdown with handl fences

Content files are Markdown. A fence extension allows handl expressions and
components inline:

````markdown
# Installation

To install handl-press:

```handl-press{component="InstallSteps" platform="linux"}
{{ kit.press.recommended_install("linux") }}
```
````

The fence is parsed by `plugin-lib-markdown` into an AST node carrying the
handl source. At render time the theme receives the node and evaluates the
handl in a scope where `kit.press` (the loaded config) and the current page's
frontmatter are bound. This is the VitePress "component in Markdown" property,
expressed as "handl in Markdown."

Frontmatter is YAML (reuses `plugin-lib-yaml`); a `---`-delimited header
matches the existing `---` doc-comment convention in `.hnl` files, so the
authoring surface is consistent across code and content.

### 3. Configuration — a handl program, not a data file

The site config is a `.hnl` file (`press.config.hnl` by convention). It is a
program that returns a `PressConfig` value:

```handl
-- press.config.hnl
import { nav-from-dir, sidebar-from-dir } from "press/nav"
import { default-theme } from "press/theme-default"

press-config = {
  title: "handl-press",
  description: "A docs framework written in handl.",
  base: "/",
  theme: default-theme {
    sidebar: sidebar-from-dir("guide", depth: 3),
    nav: nav-from-dir("."),
  },
  build: {
    out-dir: "dist",
    ssg: true,
    ssg-routes: ["**/*.md"],
  },
  cap: ["fs.read", "http.serve"],   -- explicit capability surface
}

press-config
```

This is the VitePress `config.ts` property: nav and sidebar are _computed_
from the content directory, not hand-listed. `sidebar-from-dir` and
`nav-from-dir` are handl functions in `plugin-lib-docs-config` that walk the
content tree via `fs.read` (capability-tracked) and return structured data.

The config file declares its own capabilities (`cap`). A build host refuses
to run a config whose `cap` set exceeds what the host grants — the same
mechanism ADR-0077 Phase 5 specifies for stdlib libs, applied to the docs
framework. A theme that needs no network cannot reach the network.

### 4. Theme protocol — handl functions returning HTML

A theme is a `.hnl` module exporting a set of render functions, one per
layout slot:

```handl
-- theme-default.hnl
export layout = fn(ctx) {
  tag.html {
    head: head(ctx),
    body: tag.body {
      children: [
        sidebar(ctx.config.sidebar, ctx.path),
        tag.main { children: [ctx.content] },
      ],
    },
  }
}

export head = fn(ctx) { ... }
export sidebar = fn(tree, current-path) { ... }
```

`ctx` is a record carrying `config`, `path`, `frontmatter`, `content`
(already-rendered HTML from the Markdown pass), and `slots` (named handl
values the page can inject via fences). Themes compose: a user theme can
`import { layout as base-layout } from "press/theme-default"` and wrap or
override individual slots. This is VitePress's "extend the default theme"
pattern, expressed as handl module import + record spread.

HTML emission goes through `plugin-lib-tag` (or a thin `tag` wrapper in
`plugin-lib-docs-theme`), which produces structured, escapable HTML rather
than string concatenation. The output is a `tag` value that serializes to
HTML — same data-is-output property ADR-0044 establishes for `Language`.

### 5. Build pipeline — arrows all the way down

The build is a handl arrow graph (ADR-0052), not a Rust pipeline:

```
read-content → parse-markdown → extract-frontmatter
            → resolve-routes → render-theme → write-dist
                              ↘ build-search-index (optional)
```

Each node is an `op` (ADR-0077 Phase 2) or a handl-authored arrow. The
pipeline is itself a `.hnl` file (`press.build.hnl`) so users can insert
custom passes (e.g. a "render diagrams from `.mmd` files" pass) by editing
the build arrow, without forking the framework. This is the property
VitePress gets from Vite plugins; handl gets it from "the build is a
program in the same language as the config."

### 6. Dev server — HMR via ADR-0021

`kit-press dev` runs the build arrow in watch mode (ADR-0021 HMR). Content,
config, and theme files are dependencies of the build arrow; on change, the
affected sub-graph re-evaluates and the browser reloads only the changed
slot. The dev server is `plugin-lib-http-server-std` serving the in-memory
render output — no `dist/` write in dev mode. This is the VitePress dev
experience, implemented on handl's existing HMR + HTTP server primitives.

### 7. Multi-target output

Because the framework is handl arrows + `op` declarations, the same
`press.config.hnl` produces:

- **SSG** (default): build arrow writes `dist/*.html` via `fs.write`.
- **SSR**: `http-server` serves rendered HTML per request, in-process eval.
- **Client-side**: compile the theme + content to WASM (ADR-0077 multi-target
  support, `*-wasm` impl crates) for an in-browser SPA docs site.
- **PDF / single-file**: a custom build arrow pass that serializes the route
  tree to one document.

The target is selected by which impl crates the host registers, not by
framework code. This is the ADR-0077 interface/impl split applied to the
docs framework — and it is a property VitePress does not have without a
separate SSR pipeline.

### 8. Search — optional, capability-gated

`plugin-lib-docs-search` builds a client-side index (Pagefind-style:
content sharded, index in JSON) during the build pass, and ships a
query `op` that runs against the index in the browser. The lib declares
`cap "search.build"` (build-time) and `cap "search.query"` (runtime); a
site that does not enable search simply does not register the build cap,
and the index pass is skipped. This keeps search opt-in without conditional
imports.

### 9. i18n — via `plugin-lib-intl`

Locale routing and translated UI strings use `plugin-lib-intl` directly.
Content translation is per-file (`guide.en.md`, `guide.es.md`); the route
table exposes `/en/guide` and `/es/guide` from the same `nav-from-dir`
function parameterized by locale. No framework-specific i18n mechanism is
invented.

## Consequences

### Positive

- **One language for content, config, theme, and build.** No Vue, no TS
  config, no templating DSL. A handl user already knows the framework's
  extension surface.
- **Capability-tracked themes and configs.** A theme's I/O is explicit and
  enforced (ADR-0077 Phase 3 + Phase 5). This is a security property
  VitePress approximates with bundler sandboxing and that handl gets from
  the language.
- **Multi-target output is inherent.** SSG, SSR, WASM-client, and PDF share
  one config and one theme; only the impl crates change.
- **The build is a program.** Custom passes (diagrams, link checking,
  external-data embedding) are handl arrows inserted into the build graph,
  not framework plugins in a foreign language.
- **Reuses the stdlib migration end state.** No new runtime mechanism; the
  framework is a validation of ADR-0077's interface/impl split applied to a
  real application.
- **Self-documenting.** The framework is built from `plugin-lib-docs-*` libs
  that themselves carry `---` doc comments and axioms (ADR-0077). The docs
  site can be built _by_ the framework it documents, once the framework
  exists — a small but clean bootstrap.

### Negative

- **Four new `plugin-lib-*` crates.** `plugin-lib-markdown`,
  `plugin-lib-docs-config`, `plugin-lib-docs-theme`, `plugin-lib-docs-build`,
  `plugin-lib-docs-search`. Each carries a Rust impl crate for the
  irreducible parts (Markdown block parser, search index). This is real
  surface area to maintain, gated behind the backlog.
- **Markdown parser is Rust.** A correct, fast CommonMark + GFM parser is
  not worth re-implementing in handl. The parser is a Rust `handl` (the
  handler for the `markdown.parse` `op`); the rest of the framework is
  handl. This is the same trusted-axiom boundary ADR-0077 establishes for
  `fs`, `crypto`, etc.
- **A second content format.** Markdown alongside `.hnl` means two authoring
  surfaces. This is unavoidable — prose is Markdown, not handl — but it is a
  real cognitive boundary. The handl-in-md fence softens it by making the
  boundary explicit and composable.
- **Theme protocol is a stable API.** The `ctx` record shape and the slot
  list are a public contract themes depend on. Evolving it without breaking
  user themes requires versioning (ADR-0012) and likely a `theme.v1`,
  `theme.v2` namespace.
- **Depends on ADR-0077 Phases 2–5.** `op`/`handl`/`cap` surface, eval-time
  axiom enforcement, and capability declaration must all land before the
  framework can be built on them rather than alongside them. Building early
  would re-create the per-library Rust scaffolding ADR-0077 eliminates.

### Neutral

- **VitePress is the reference, not the target.** Feature parity is not a
  goal; the goal is the VitePress _class_ of framework (content + config +
  components + SSG + dev server) expressed in handl's idioms. Features
  VitePress has that do not fit handl's model (e.g. Vue-specific reactivity)
  are not replicated; handl's arrow model replaces them where a replacement
  is needed.

## Implementation phases

Each phase is gated on the corresponding ADR-0077 phase. None of these
phases start until ADR-0077 Phase 4 (native lib migration) is substantially
complete; the framework is a consumer of that work.

### Phase 0 — Protocol sketch (no code)

Define the `PressConfig` record shape, the theme `ctx` record shape, the
build arrow graph, and the `op` set (`markdown.parse`, `press.render-page`,
`press.build-route`, `search.build`, `search.query`). Record as a `.hnl`
interface file with `---` doc and axioms but no impl. This fixes the
contract before any Rust is written.

### Phase 1 — `plugin-lib-markdown`

Rust CommonMark/GFM parser (likely `pulldown-cmark` or equivalent) exposed
as `op "markdown.parse" : String -> MarkdownAst`. Handl-in-md fences are a
parser extension that emits a `HandlFence` AST node carrying the source.
Axiom: `guarantee: IsValidMarkdown(output)`.

### Phase 2 — `plugin-lib-docs-config`

Handl-authored `nav-from-dir`, `sidebar-from-dir`, `route-table-from-dir`
functions. The `PressConfig` schema as a `type` declaration (ADR-0008).
Validation arrows that check the config against the schema at load time.

### Phase 3 — `plugin-lib-docs-theme` + default theme

The theme protocol (`layout`, `head`, `sidebar`, `content` slots) and a
default theme implementing it. HTML emission via `plugin-lib-tag`. This is
the first phase that produces a renderable site.

### Phase 4 — `plugin-lib-docs-build` + dev server

The build arrow graph (SSG) and the dev server (HMR via ADR-0021). End of
this phase: `kit-press dev` and `kit-press build` both work end-to-end on a
sample site.

### Phase 5 — `plugin-lib-docs-search` (optional)

Client-side index build + query `op`. Gated behind a config flag; sites
that do not enable search do not register the build cap.

### Phase 6 — Multi-target (SSR, WASM client)

Swap impl crates to run the same config as SSR (`http-server` in-process)
and as a WASM client-side SPA. Validates the ADR-0077 interface/impl split
on a real application.

## References

- [ADR-0077](./0077-stdlib-migration-to-handl.md) — Stdlib Migration to
  Handl. The `op`/`handl`/`cap` surface, eval-time axiom enforcement, and
  interface/impl split this framework is built on.
- [ADR-0069](./0069-arrow-native-modules.md) — Arrow-Native Modules. The
  importable `.hnl` module system the framework's libs and themes use.
- [ADR-0052](./0052-arrow-calculus-kernel.md) — Arrow Calculus Kernel. The
  build pipeline is an arrow graph.
- [ADR-0021](./0021-hmr.md) — HMR. The dev server's hot reload.
- [ADR-0079](./0079-unify-dialects-no-let-in.md) — Unified Dialect. Graph
  blocks as the configuration surface.
- [ADR-0044](./0044-language-as-data-and-runtime-split.md) — Language as
  Data. The "output is data" property the theme's `tag` values rely on.
- [ADR-0039](./0039-kits-are-curated-plugin-lists.md) — Kits. `kit-press`
  is a curated plugin list, not a monolith.
- [ADR-0012](./0012-versioning.md) — Versioning. The theme protocol is a
  versioned API.
