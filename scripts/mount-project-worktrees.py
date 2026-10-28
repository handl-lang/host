#!/usr/bin/env python3
"""Mount a `project/` worktree in every sibling repo (ADR-0129 §1).

For each repo under the org root:
  * `/project/` is in `.gitignore` (main stays a clean build tree);
  * an orphan `project` branch exists and is pushed;
  * it is checked out as the `project/` worktree, seeded with the map.

Idempotent. Dry-run by default; `--apply` performs the changes and pushes.

Usage:
    python3 scripts/mount-project-worktrees.py --root ../.. --dry-run
    python3 scripts/mount-project-worktrees.py --root ../.. --apply --only engine-*
"""

from __future__ import annotations

import argparse
import fnmatch
import subprocess
import sys
from pathlib import Path

IGNORE_LINE = "/project/"
GITIGNORE_COMMIT = "chore: ignore the project worktree (ADR-0129)"
SEED_COMMIT = "docs: initialize the project branch (ADR-0129)"

SEED = """# `project/` — {repo}

Docs for this repo live on the `project` branch, mounted here as a worktree.
`main` stays a clean build tree; this directory is gitignored.

| entry | answers |
|---|---|
| `adrs/` | *why* — decisions, and what was rejected |
| `design/` | *how* — mechanism, named after its ADR |
| `tasks/`, `TODO.md` | *who/what next* |
| `notes/` | *what we found* |
| `reports/` | *what we measured* (dated) |
| `BACKLOG.md` | *not now* |
| `references.md` | *what we depend on* — foreign ADR ids + URLs |

Layout rule: ADR-0162 (`host` repo, `project` branch). ADR shape: ADR-0161.
Published documentation (tutorial, how-to, reference, explanation) belongs to
the org `docs` repo, not here.
"""


def git(repo: Path, *args: str, check: bool = False) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(repo), *args],
                          capture_output=True, text=True, check=check)


def has_ref(repo: Path, ref: str) -> bool:
    return git(repo, "rev-parse", "--verify", "-q", ref).returncode == 0


def tracked_on_main(repo: Path) -> bool:
    out = git(repo, "ls-tree", "-d", "--name-only", "HEAD", "project")
    return bool(out.stdout.strip())


def state(repo: Path) -> dict:
    wt = (repo / "project" / ".git").is_file()
    gi = (repo / ".gitignore").read_text(errors="replace").splitlines() \
        if (repo / ".gitignore").is_file() else []
    return {
        "worktree": wt,
        "ignored": IGNORE_LINE in [line.strip() for line in gi],
        "branch": has_ref(repo, "refs/heads/project"),
        "remote": has_ref(repo, "refs/remotes/origin/project"),
        "tracked": tracked_on_main(repo),
    }


def classify(s: dict) -> str:
    if s["tracked"]:
        return "tracked-on-main"
    if s["worktree"] and s["ignored"] and s["branch"]:
        return "ok"
    return "needs-mount"


def mount(repo: Path, s: dict, apply: bool) -> list[str]:
    done: list[str] = []
    if not s["ignored"]:
        done.append("gitignore")
        if apply:
            with (repo / ".gitignore").open("a") as fh:
                fh.write(f"\n{IGNORE_LINE}\n")
            git(repo, "add", ".gitignore")
            git(repo, "commit", "-q", "-m", GITIGNORE_COMMIT)
            git(repo, "push", "-q", "origin", "HEAD")
    if not s["worktree"]:
        done.append("worktree")
        if apply:
            if s["remote"] and not s["branch"]:
                git(repo, "fetch", "-q", "origin", "project:project")
            if s["branch"] or has_ref(repo, "refs/heads/project"):
                git(repo, "worktree", "add", str(repo / "project"), "project")
            else:
                git(repo, "worktree", "add", "--orphan", "-b", "project",
                    str(repo / "project"))
                seed = repo / "project" / "README.md"
                seed.write_text(SEED.format(repo=repo.name), encoding="utf-8")
                git(repo / "project", "add", "README.md")
                git(repo / "project", "commit", "-q", "-m", SEED_COMMIT)
                git(repo / "project", "push", "-q", "-u", "origin", "project")

    return done


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--root", type=Path, required=True, help="org checkout root")
    ap.add_argument("--apply", action="store_true", help="perform and push")
    ap.add_argument("--dry-run", action="store_true", help="report only (default)")
    ap.add_argument("--only", default="*", help="glob on repo name, e.g. 'engine-*'")
    args = ap.parse_args(argv)

    root = args.root.resolve()
    if not root.is_dir():
        print(f"no such root: {root}", file=sys.stderr)
        return 1

    counts = {"ok": 0, "needs-mount": 0, "tracked-on-main": 0}
    todo: list[tuple[str, str, list[str]]] = []

    for repo in sorted(p for p in root.iterdir() if p.is_dir()):
        if not (repo / ".git").exists():
            continue
        if not fnmatch.fnmatch(repo.name, args.only):
            continue
        s = state(repo)
        kind = classify(s)
        counts[kind] += 1
        if kind == "ok":
            continue
        if kind == "tracked-on-main":
            todo.append((repo.name, kind, ["manual: git rm -r --cached project"]))
            continue
        todo.append((repo.name, kind, mount(repo, s, args.apply)))

    verb = "APPLIED" if args.apply else "DRY-RUN"
    print(f"{verb}  root={root}  pattern={args.only}")
    print(f"  ok: {counts['ok']}   needs-mount: {counts['needs-mount']}   "
          f"tracked-on-main: {counts['tracked-on-main']}\n")
    for name, kind, actions in todo[:40]:
        print(f"  {name:<24} {kind:<16} {', '.join(actions)}")
    if len(todo) > 40:
        print(f"  … {len(todo) - 40} more")
    if args.apply or not todo:
        return 0
    print("\nre-run with --apply to perform these changes (each pushes a "
          "gitignore commit and a project branch)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
