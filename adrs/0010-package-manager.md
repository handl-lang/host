---
title: 'Package Manager: Content-Addressed, Diff-Based, Composable Stores'
type: adr
date: 2026-08-24
status: proposed
---

# Package Manager: Content-Addressed, Diff-Based, Composable Stores

## Context

The original ADR-0010 proposed `handl install` with a pluggable registry
protocol writing whole package trees into a project-local
`.handl/packages/` directory, pinned by semver version (ADR-0012).
Experience and the platform's direction toward graph-based, data-driven
artifacts (ADR-0052, ADR-0058) expose several problems with that model:

- **Copying is wasteful.** A handl package is a graph of arrows/documents
  plus mapped artifacts (docs, source maps, typegen output). Reinstalling
  or updating a package copies the entire tree, even when most nodes are
  unchanged.
- **Project-only scope forces duplication.** Every project re-installs the
  same shared libraries into its own `.handl/packages/`. There is no
  user-wide pool, so common kits are stored N times.
- **Versioning as identity is brittle.** Semver `version` fields
  (ADR-0012) name a release, not the content. Two builds of "1.2.0" can
  differ; the resolver trusts a string instead of the bytes. Pinning by
  tag without verifying content is unsafe.
- **Layout is hardcoded.** The on-disk format (a directory tree under
  `.handl/packages/`) is fixed, so hosts and power users cannot substitute
  a faster or shared backend.

handl packages are graphs, not file trees. The install mechanism should
treat them as such: content-addressed, diff-applied, and composable across
scopes. This is very close to git's model, but for packages of
arrows/handl code.

## Decision

`handl install` operates on **content-addressed package graphs** stored in
**composable package stores**. Identity is a content hash; tags are
organizational pointers. Installation applies graph diffs, not file
copies.

### Package identity: hashes and tags

A package is a graph of handl/arrow nodes plus mapped artifacts (docs,
source maps, derived outputs). Its identity is the **content hash** of its
graph root — a Merkle-style hash over node hashes. Hashes are immutable
and content-verified on every fetch.

**Tags** are named, mutable pointers to hashes, used only for human
organization (`v1.2.0`, `latest`, `stable`). A specifier like
`9aia/deploy-patterns@v1.2.0` resolves the tag to a hash at install time;
the hash is what gets pinned. Tags may move; pinned hashes do not.

This **supersedes ADR-0012**: the semver `version` field is no longer
package identity. `@v1.2.0` in a specifier is a tag lookup, not a version
match. Documents may keep a `version` field as metadata only; the resolver
does not use it for identity or pinning.

### Package store trait

The on-disk layout is abstracted behind a `PackageStore` trait. The
default implementation is a **SQLite-backed store at the user's home**
(`~/.handl/store.db`, respecting `XDG` data dirs).

```rust
pub trait PackageStore {
    fn contains(&self, hash: &ContentHash) -> bool;
    fn materialize(&self, hash: &ContentHash) -> Result<MaterializedGraph>;
    fn apply_diff(&self, base: Option<&ContentHash>, diff: &GraphDiff) -> Result<ContentHash>;
    fn resolve_tag(&self, tag: &TagSpec) -> Result<ContentHash>;
    fn set_tag(&self, tag: &TagSpec, hash: &ContentHash) -> Result<()>;
    fn list(&self) -> Result<Vec<StoreEntry>>;
}

pub struct ContentHash(pub [u8; 32]); // sha256

pub struct GraphDiff {
    pub added: Vec<GraphNode>,
    pub removed: Vec<ContentHash>,
    pub updated: Vec<(ContentHash, GraphNode)>,
}
```

The store holds content-addressed nodes, diffs, tag mappings, and a
manifest. Artifacts (docs, source maps, typegen) are stored as
content-addressed blobs referenced from graph nodes — **mapped, not
copied per install**.

### Default scope: user-wide, opt-out

`handl install` writes to the **user-wide store by default**. Installing
at a project or arbitrary folder is opt-out:

```bash
handl install 9aia/deploy-patterns          # → user store
handl install 9aia/deploy-patterns --into . # → project store (.handl/store.db)
handl install 9aia/deploy-patterns --into /opt/shared/kits
```

### Composable layouts

Stores compose. The resolver searches an ordered chain: project-local
`.handl/store.db` first, then the user-wide store. A package present in
either is importable. This mirrors `PATH` semantics: a project can pin a
specific hash in its local store that shadows the user-wide one, while
still inheriting everything else.

```rust
pub struct ComposedStore {
    layers: Vec<Box<dyn PackageStore>>, // project-local → user-wide → ...
}
```

Composition saves space: a subgraph shared by two packages is stored once
and referenced by both. When layers share a backing store, dedup is
automatic across scopes; within a single store, content addressing makes
every shared node a single copy.

### Diff-based installation

Installing a package does not copy a file tree. The registry serves a
**graph diff** against a base the store already has (or an empty base).
The store applies the diff and records the resulting root hash.

- Re-installing the same hash is a no-op.
- Updating `v1.2.0 → v1.3.0` fetches only the diff between the two graph
  roots; unchanged nodes are reused.
- Docs, source maps, and derived artifacts are content-addressed blobs
  referenced from nodes — they are fetched once and shared, not re-copied
  per package.

### Registry trait

Registries adapt external sources (git, local dirs, URLs) into graph
diffs and tag mappings. The trait is reshaped from the original ADR-0010
file-copy model:

```rust
pub trait Registry {
    fn can_resolve(&self, spec: &str) -> bool;
    fn resolve(&self, spec: &str, store: &dyn PackageStore) -> Result<ContentHash>;
}
```

`resolve` fetches the source, computes the package graph, diffs it against
what `store` already holds, applies the diff, and returns the pinned root
hash. Default registries (LocalFile, GitHub, GitLab, GenericGit, URL) are
retained from the original ADR-0010 but each produces graphs/diffs instead
of directory copies.

### Lockfile

`.handl/lock.json` pins **hashes** (with the tag they resolved from, for
readability):

```json
{
  "packages": {
    "9aia/deploy-patterns": {
      "hash": "sha256:abc123...",
      "tag": "v1.2.0",
      "scope": "user",
      "source": "github:9aia/deploy-patterns"
    }
  }
}
```

Reproducibility comes from the hash, not the tag. A moved tag is
detectable: if the lockfile's tag now resolves to a different hash,
`handl install --check` reports the drift.

### Command surface

```bash
handl install 9aia/deploy-patterns
handl install 9aia/deploy-patterns@v1.2.0
handl install 9aia/deploy-patterns --into .
handl install gitlab:acme/campaigns
handl install git+https://git.example.com/corp/patterns
handl install --list
handl install --check        # report tag drift vs lockfile
```

## Consequences

- **Packages are graphs, not trees.** Install, update, and materialization
  operate on graph diffs; unchanged nodes and artifacts are never
  re-copied.
- **User-wide by default** eliminates redundant per-project copies of
  shared kits; project-local installs are opt-out for when a project needs
  to pin or isolate.
- **Composable stores** let a project shadow specific packages while
  inheriting the user pool, with content-addressed dedup saving space
  across scopes.
- **Content hashes are identity.** Integrity is verified on every fetch;
  tag drift is detectable. This supersedes ADR-0012's semver-as-identity
  model — `version` becomes metadata, not a pin.
- **Layout is pluggable.** The SQLite default can be replaced by a
  host-specific or shared/networked store via the `PackageStore` trait.
- **Fully cacheable.** Content-addressed nodes and diffs make every
  install idempotent and every update proportional to the change, not the
  package size.
- **No code execution on install** (retained from the original ADR-0010).
  Install fetches and applies graph data only.
- **ADR-0012 is superseded.** Its `version` field and `name@version`
  semver matching are replaced by hash identity + tag pointers. The
  `@v1.2.0` specifier syntax is retained as a tag lookup.
- New crates: `handl-pkg-store` (trait + SQLite default),
  `handl-pkg-diff` (graph diff/apply), `handl-pkg-registry-*` (per-source
  adapters). The original `handl-pkg-install` directory-copy design is
  replaced.

## Status

Proposed
