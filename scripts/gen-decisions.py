#!/usr/bin/env python3
"""Generate `project/DECISIONS.md` — one line per decision rule (ADR-0161 §8).

The recall surface: every ADR's `## Decision` collapses to one line per rule,
with a link back to the argument. Generated from every sibling repo's
`project/adrs/` (archived records excluded), so the file is never hand-edited.

Usage:
    python3 scripts/gen-decisions.py            # writes project/DECISIONS.md
    python3 scripts/gen-decisions.py --check     # exit 1 if it would change
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent          # the host repo
ORG = ROOT.parent
OUT = ROOT / "project" / "DECISIONS.md"

NUM = re.compile(r"^(\d+[a-z]?)-\w")
H1 = re.compile(r"^#\s+(.+)$", re.M)
DECISION = re.compile(r"^##\s+Decision\b(.*?)(?=^##\s|\Z)", re.M | re.S)
ITEM = re.compile(r"^\s*(?:\d+\.|[-*])\s+(.*)$")
FRONT = re.compile(r"\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*\r?\n", re.S)


def frontmatter(text: str) -> dict:
    m = FRONT.match(text)
    if not m:
        return {}
    try:
        data = yaml.safe_load(m.group(1)) or {}
    except yaml.YAMLError:
        return {}
    return data if isinstance(data, dict) else {}


def one_line(text: str, limit: int = 120) -> str:
    text = re.sub(r"`([^`]*)`", r"\1", text)
    text = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"\s+", " ", text).strip(" .")
    if len(text) > limit:
        cut = text[:limit]
        text = cut.rsplit(" ", 1)[0] + "…"
    return text


def rules_of(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    m = DECISION.search(text)
    if not m:
        return []
    rules: list[str] = []
    for line in m.group(1).splitlines():
        hit = ITEM.match(line)
        if hit:
            rule = one_line(hit.group(1))
            if rule and rule not in rules:
                rules.append(rule)
    return rules


def remote_name(repo_dir: Path) -> str:
    try:
        url = subprocess.run(
            ["git", "-C", str(repo_dir), "remote", "get-url", "origin"],
            capture_output=True, text=True, check=True).stdout.strip()
    except subprocess.CalledProcessError:
        return repo_dir.name
    name = url.rsplit("/", 1)[-1]
    return name[:-4] if name.endswith(".git") else name


def adr_files(proj: Path) -> list[Path]:
    d = proj / "adrs"
    return [p for p in sorted(d.glob("*.md")) if NUM.match(p.name)] if d.is_dir() else []


def render(proj: Path, remote: str) -> str:
    rows = []
    for path in adr_files(proj):
        text = path.read_text(encoding="utf-8", errors="replace")
        fm = frontmatter(text)
        status = str(fm.get("status") or "").strip()
        if status in ("superseded", "absorbed"):
            continue                     # history, not the recall surface
        title = str(fm.get("title") or "").strip()
        if not title:
            h1 = H1.search(text)
            title = one_line(h1.group(1)) if h1 else path.stem
        rules = rules_of(path)
        if not rules:
            continue
        num = NUM.match(path.name).group(1)
        link = f"adrs/{path.name}"
        body = "\n".join(f"- {rule} — [ADR-{num}]({link})" for rule in rules)
        rows.append((num.rjust(5, "0"), f"### {title}\n\n{body}"))
    rows.sort()
    return (
        "---\ntitle: Decisions\ntype: index\n---\n\n"
        "# Decisions\n\n"
        "One line per decision rule, generated from each ADR's `## Decision`\n"
        "section with `host/scripts/gen-decisions.py`. The ADR is the argument;\n"
        "this is the recall surface (ADR-0161 §8). Superseded records are excluded.\n\n"
        + "\n\n".join(r[1] for r in rows) + "\n"
    )


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--check", action="store_true",
                    help="exit 1 if any file is out of date")
    args = ap.parse_args(argv)
    stale = 0
    for repo in sorted(ORG.iterdir()):
        proj = repo / "project"
        if not proj.is_dir() or not adr_files(proj):
            continue
        text = render(proj, remote_name(repo))
        out = proj / "DECISIONS.md"
        current = out.read_text(encoding="utf-8") if out.exists() else ""
        if current != text:
            stale += 1
            if args.check:
                print(f"{out} is out of date", file=sys.stderr)
            else:
                out.write_text(text, encoding="utf-8")
                print(f"wrote {out.relative_to(ORG)} — {len(text.encode())} bytes")
        elif not args.check:
            print(f"current {out.relative_to(ORG)}")
    if args.check:
        return 1 if stale else 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
