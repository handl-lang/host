#!/usr/bin/env python3
"""Pin cross-repo Cargo git dependencies to a `rev` (ADR-0128 §Resolution).

For every sibling repo, rewrite `git = "https://github.com/handl-lang/<dep>"`
in its `Cargo.toml`s to add `rev = "<dep's current HEAD sha>"`. A tag is not
needed — `rev` is immutable and Cargo resolves it exactly.

    scripts/pin-git-revs.py            pin (rewrite in place)
    scripts/pin-git-revs.py --check    report unpinned/out-of-date only

`[patch]` tables still win during local development, so sibling checkouts are
unaffected.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ORG = Path(__file__).resolve().parent.parent.parent
BASE = "https://github.com/handl-lang/"
PAT = re.compile(
    r'git = "' + re.escape(BASE) + r'(?P<repo>[A-Za-z0-9._-]+)"'
    r'(?:, rev = "(?P<rev>[0-9a-f]+)")?'
)


def head_sha(repo: str) -> str | None:
    d = ORG / repo
    if not (d / ".git").exists():
        return None
    try:
        return subprocess.run(
            ["git", "-C", str(d), "rev-parse", "HEAD"],
            capture_output=True, text=True, check=True,
        ).stdout.strip()
    except subprocess.CalledProcessError:
        return None


def main(argv: list[str]) -> int:
    check = "--check" in argv
    repos = sorted(d for d in ORG.iterdir() if (d / ".git").is_dir())
    shas = {d.name: head_sha(d.name) for d in repos}

    changes: list[str] = []
    unpinned: list[str] = []

    for d in repos:
        for toml in d.rglob("Cargo.toml"):
            if "target/" in str(toml) or "node_modules" in str(toml):
                continue
            text = toml.read_text()
            if BASE not in text:
                continue

            def sub(m: re.Match) -> str:
                dep = m.group("repo")
                sha = shas.get(dep)
                if sha is None:
                    unpinned.append(f"{toml}: dep '{dep}' has no local checkout")
                    return m.group(0)
                if m.group("rev") == sha:
                    return m.group(0)
                return f'git = "{BASE}{dep}", rev = "{sha}"'

            new = PAT.sub(sub, text)
            if new != text:
                rel = toml.relative_to(ORG)
                changes.append(str(rel))
                if not check:
                    toml.write_text(new)

    for line in unpinned:
        print(f"WARN  {line}")
    for c in changes:
        print(f"{'WOULD UPDATE' if check else 'updated'}  {c}")
    print(f"\n{len(changes)} file(s) {'need updating' if check else 'updated'}, "
          f"{len(unpinned)} warning(s)")
    return 1 if (check and changes) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
