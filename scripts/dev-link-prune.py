#!/usr/bin/env python3
"""Drop `[patch]` entries of a repo's root Cargo.toml that cargo never used.

Why
---
`dev-link.sh` patches *every* crate of every sibling checkout the repo
git-depends on.  Crates nothing in the graph depends on still get a patch
entry, and cargo echoes each of those into `Cargo.lock` as
`[[patch.unused]]`.

Cargo serializes `[[patch.unused]]` in HashMap iteration order, so the
lockfile is byte-unstable between runs.  As long as the unused entries come
from a single patch source (the global `~/.cargo/config.toml` sepia
patches) the ordering is stable in practice; once *two* sources contribute
(any manifest entry here + global config), every re-resolution writes a
different file and `cargo … --locked` refuses forever:

    error: cannot update the lock file … because --locked was passed

Unused patch entries do not participate in resolution, so removing them is
neutral: same dependency graph, same versions, deterministic lockfile.

Usage
-----
    dev-link-prune.py <repo-dir> [--dry-run]

Reads `<repo-dir>/Cargo.lock` for the unused set and rewrites
`<repo-dir>/Cargo.toml` in place (unless --dry-run).  Prints
`removed=<n> tables=<n> remaining-unused=<n>`; exits 1 on a usage error.

Re-run `cargo metadata` afterwards: the lockfile still carries the
now-deleted entries until it is re-resolved.
"""
import pathlib
import re
import sys


def unused_patch_names(lock_text):
    """Package names cargo reported under [[patch.unused]]."""
    out = set()
    inside = False
    for line in lock_text.splitlines():
        if line.strip() == "[[patch.unused]]":
            inside = True
            continue
        if inside:
            if line.startswith("["):
                inside = False
                continue
            m = re.match(r'name\s*=\s*"([^"]+)"', line.strip())
            if m:
                out.add(m.group(1))
    return out


def prune(toml_text, unused):
    """Drop member lines naming an unused patch; drop emptied tables.

    A name that appears in more than one `[patch."…"]` table is *skipped*:
    `[[patch.unused]]` records only `name` + `version` (no source URL), so
    cargo cannot tell us which of the duplicates it meant, and dropping the
    used one would silently unpatch a real dependency.
    """
    lines = toml_text.splitlines(keepends=True)
    section = re.compile(r"^\s*\[")
    patch_header = re.compile(r"^\s*\[patch\.")
    member = re.compile(r"^\s*([A-Za-z0-9_-]+)\s*=")

    seen = {}
    in_patch = False
    for line in lines:
        if section.match(line):
            in_patch = bool(patch_header.match(line))
            continue
        m = member.match(line)
        if in_patch and m and not line.lstrip().startswith("#"):
            seen[m.group(1)] = seen.get(m.group(1), 0) + 1
    ambiguous = {name for name, n in seen.items() if n > 1}
    removable = unused - ambiguous

    out = []
    removed = tables = 0
    i, n = 0, len(lines)
    while i < n:
        line = lines[i]
        if not patch_header.match(line):
            out.append(line)
            i += 1
            continue
        j = i + 1
        body = []
        while j < n and not section.match(lines[j]):
            body.append(lines[j])
            j += 1
        kept = []
        members = 0
        for b in body:
            m = member.match(b)
            if m and not b.lstrip().startswith("#"):
                members += 1
                if m.group(1) in removable:
                    removed += 1
                    continue
            kept.append(b)
        if members and not any(
            member.match(k) and not k.lstrip().startswith("#") for k in kept
        ):
            # Every member was unused: drop the table header, but keep the
            # comment/blank lines that trailed it — the `# dev-link: end`
            # marker lives in that tail, and losing it makes the next
            # `dev-link.sh` run append a second region (duplicate keys).
            tables += 1
            out.extend(kept)
            i = j
            continue
        out.append(line)
        out.extend(kept)
        i = j

    text = re.sub(r"\n{3,}", "\n\n", "".join(out))
    return text, removed, tables, len(ambiguous & unused)


def main(argv):
    args = [a for a in argv[1:] if not a.startswith("-")]
    dry_run = "--dry-run" in argv[1:]
    if len(args) != 1:
        print(__doc__.strip().splitlines()[0], file=sys.stderr)
        print("usage: dev-link-prune.py <repo-dir> [--dry-run]", file=sys.stderr)
        return 1

    repo = pathlib.Path(args[0])
    toml_p, lock_p = repo / "Cargo.toml", repo / "Cargo.lock"
    if not lock_p.is_file():
        print(f"dev-link-prune: {lock_p} not found — nothing to prune", file=sys.stderr)
        return 1

    unused = unused_patch_names(lock_p.read_text())
    text, removed, tables, skipped = prune(toml_p.read_text(), unused)
    if not dry_run and removed:
        toml_p.write_text(text)
    print(
        f"removed={removed} tables={tables} "
        f"remaining-unused={len(unused)} skipped-ambiguous={skipped}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
