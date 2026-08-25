# handl_module_resolver_git

# handl-module-resolver-git

Git-based [`ModuleResolver`] implementation.

Fetches documents from Git repositories using `git2`. Module paths use
the `git+` URL scheme:

```text
import('git+https://github.com/org/repo.git@main/schemas/base.yml')
```

Repositories are cloned to a cache directory keyed by a SHA-256 hash
of `(repo_url, ref)`. Clones are **bare** and **shallow** (depth=1 by
default) to minimize download size. Subsequent resolves from the same
repo+ref fetch updates from the remote before reading.

## Modules

### [`handl_module_resolver_git`](handl_module_resolver_git.md)

*3 modules*

### [`fetch`](fetch.md)

*1 constant, 1 struct*

### [`fetch::GitFetcher`](fetch/GitFetcher.md)

*2 functions*

### [`git_resolver`](git_resolver.md)

*1 struct*

### [`parse`](parse.md)

*1 function, 1 struct*

