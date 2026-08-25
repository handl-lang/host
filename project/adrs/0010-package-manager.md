# Package Manager and Registry Protocol

## Context

handl documents reference external modules via `import()`. Today, module
resolution is limited to local file paths. There is no way to install,
version, or share handl modules across projects without manually copying
files or vendoring repositories.

A package manager lets authors discover, install, and pin external modules.
This is a language platform concern — the package manager operates on
handl modules (`.handl`, `.yaml`, `.json`, `.toml` documents and libraries),
not on host-specific runtime artifacts. Hosts (including Peregrin) may
layer their own package types on top, but the core install/resolve mechanism
belongs to the language toolchain.

## Decision

handl provides a `handl install` command and a pluggable registry protocol.

### Registry trait

```rust
pub trait Registry {
    fn can_resolve(&self, spec: &str) -> bool;
    fn resolve(&self, spec: &str, into: &Path) -> Result<ResolvedPackage, RegistryError>;
}

pub struct ResolvedPackage {
    pub local_path: PathBuf,
    pub canonical_name: String,
    pub version: Option<String>,
    pub source_spec: String,
}
```

`can_resolve` is a cheap check based on the specifier shape. `resolve`
fetches the package and writes it into the local project at `into`.

### Default registry implementations

1. **LocalFileRegistry** — specifier is a relative or absolute file path.
   Resolution: copy or symlink the directory.
2. **GitHubRegistry** — specifier `org/repo` or `github:org/repo[@ref]`.
   Resolution: clone or sparse-checkout at the given ref.
3. **GitLabRegistry** — specifier `gitlab:group/project[@ref]`.
4. **GenericGitRegistry** — specifier `git+https://host/path[@ref]`.
5. **URLRegistry** (future) — specifier is an HTTP(S) URL to an archive.

### Multi-registry composition

The default install service is configured with an ordered array of
registries. For each `handl install` call, the service asks each registry
`canResolve(spec)` in order and uses the first that returns `true`. Users
override the array via project config.

### Install layout

`handl install` writes packages into a project-local directory (e.g.
`.handl/packages/`). The module resolver scans this directory alongside
project paths. An installed package is imported normally:

```handl
import("9aia/patterns/deploy")
```

A lockfile (`.handl/lock.json`) records the resolved commit, tag, or path for
each installed package so that resolution is repeatable.

### Command surface

```bash
handl install 9aia/deploy-patterns
handl install 9aia/deploy-patterns@v1.2.0
handl install filepath ../team-patterns
handl install gitlab:acme/campaigns --into packages/shared
handl install git+https://git.example.com/corp/patterns
handl install --list
```

## Consequences

- handl modules can be shared as git repositories or local directories.
- `import()` becomes more powerful: it can reference installed packages as
  well as local files.
- The module resolver gains a stable target directory for external packages.
- New crates (`handl-pkg-install`, `handl-pkg-registry-github`,
  `handl-pkg-registry-local`) keep the install logic and individual registry
  implementations separate.
- `handl install` does not execute code; it only fetches and lays out files.
- The lockfile makes shared packages deterministic without vendoring entire
  repository histories.
- Depends on ADR-0012 (Versioning) for the `name@version` specifier grammar
  and lockfile format.

## Status

Proposed
