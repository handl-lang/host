#!/usr/bin/env python3
"""Check ADRs against the one-page decision-record rules (ADR-0161).

An ADR is a *decision record*, not a design document. It must stay short
enough to be re-read in a minute, and it must carry the two things a
short record would otherwise lose:

    Why          the failure this prevents — the memory of the mistake
    Not doing    the alternatives rejected, one line each, so nobody re-proposes them

Report-only by default so the historical corpus can migrate incrementally;
`--strict` turns warnings into a failure for CI; `--changed REV` limits the run to
ADRs touched since REV (the PR ratchet); `--strict-budget` also
fails on legacy size/decision budgets (ADR-0161 grandfathers those until the
record is next edited).

Usage:
    python3 scripts/check-adrs.py
    python3 scripts/check-adrs.py --strict
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent          # the repo (e.g. host/)
PROJECT = ROOT / "project"
NUM = re.compile(r"^(\d+[a-z]?)-")
STATUS_SECTION = re.compile(r"^##\s+Status\s*$", re.M)
STATUS_LINE = re.compile(r"^[-*]?\s*\**Status\**\s*[:—]\s*(.+)$", re.M)
SUPERSEDED_BY = re.compile(r"Superseded by\s*\[?ADR[-\s]?(\d{3,4})", re.I)
DECISION = re.compile(r"^##\s+Decision\b(.*?)(?=^##\s|\Z)", re.M | re.S)
ITEM = re.compile(r"^\s*(?:\d+\.|[-*])\s+", re.M)
FRONT = re.compile(r"\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*\r?\n", re.S)

# Headings accepted as the "why" — legacy files use Context/Rationale.
WHY = re.compile(r"^##\s+(Why|Context|Rationale|Motivation)\b", re.M | re.I)
NOT_DOING = re.compile(r"^##+\s+(Not doing|Alternatives|Rejected|Considered)\b", re.M | re.I)
ENFORCEMENT = re.compile(r"^##+\s+(Enforcement|Enforced by|Verification)\b", re.M | re.I)

# ADR-0129 §3: numbers are never renumbered or reused, because they are cited
# from code. Collisions created before the global registry therefore stay as
# they are — they are recorded, not fixed. Only a *new* collision is an error.
HISTORICAL_DUPS = {"1", "2", "9", "83", "100"}


def changed_adrs(proj: Path, rev: str) -> set[Path]:
    """ADRs touched since `rev` (plus untracked ones), resolved to real paths."""
    paths: set[Path] = set()
    for args in (["diff", "--name-only", rev, "--", "."],
                 ["ls-files", "--others", "--exclude-standard", "--", "."]):
        out = subprocess.run(["git", "-C", str(proj)] + args,
                             capture_output=True, text=True).stdout
        for line in out.splitlines():
            if line.endswith(".md"):
                paths.add((proj / line).resolve())
    return paths


def adr_files(proj: Path) -> list[Path]:
    out: list[Path] = []
    for sub in ("adrs", "adrs/archived"):
        d = proj / sub
        if d.is_dir():
            out += [p for p in sorted(d.glob("*.md")) if NUM.match(p.name)]
    return out


def frontmatter(text: str) -> dict:
    """The ADR-0163 header, or {} for a legacy record."""
    m = FRONT.match(text)
    if not m:
        return {}
    try:
        data = yaml.safe_load(m.group(1)) or {}
    except yaml.YAMLError:
        return {}
    return data if isinstance(data, dict) else {}


def status_of(text: str) -> str:
    """`status` from frontmatter (ADR-0163), else `- Status: X` / `## Status`."""
    fm = frontmatter(text)
    if fm.get("status"):
        status = str(fm["status"])
        if fm.get("status_note"):
            status = f"{status} — {fm['status_note']}"
        return status
    m = STATUS_LINE.search(text)
    if m:
        return m.group(1).strip()
    m = STATUS_SECTION.search(text)
    if m:
        tail = text[m.end():].lstrip("\n").splitlines()
        return tail[0].strip() if tail else ""
    return ""


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--strict", action="store_true", help="exit 1 on any violation")
    ap.add_argument("--strict-budget", action="store_true",
                    help="also fail on legacy size/decision budgets (ADR-0161 grandfathers them)")
    ap.add_argument("--max-kb", type=int, default=8, help="soft budget per ADR (default 8)")
    ap.add_argument("--max-items", type=int, default=10, help="soft budget per Decision list")
    ap.add_argument("--repo", type=Path, default=ROOT, help="repo root (default: this repo)")
    ap.add_argument("--changed", metavar="REV",
                    help="only ADRs changed since REV (the PR ratchet)")
    ap.add_argument("--allow-dup", default="",
                    help="comma-separated historically-colliding numbers for this repo")
    args = ap.parse_args(argv)

    proj = (args.repo / "project").resolve()
    allowed = HISTORICAL_DUPS | {n.strip().lstrip("0") or "0"
                                 for n in args.allow_dup.split(",") if n.strip()}
    # `adrs/.duplicates` records pre-registry collisions for this repo: one
    # number per line, `#` comments. Never add a *new* collision here.
    dup_file = proj / "adrs" / ".duplicates"
    if dup_file.is_file():
        for line in dup_file.read_text(encoding="utf-8").splitlines():
            line = line.split("#", 1)[0].strip()
            if line:
                allowed.add(line.lstrip("0") or "0")
    files = adr_files(proj)
    if args.changed:
        touched = changed_adrs(proj, args.changed)
        files = [p for p in files if p.resolve() in touched]
        if not files:
            print(f"no ADRs changed since {args.changed}")
            return 0
    if not files:
        print(f"no ADRs under {proj}/adrs", file=sys.stderr)
        return 1

    errors: list[str] = []
    warns: list[str] = []
    historical: list[str] = []
    numbers: dict[str, list[str]] = defaultdict(list)
    oversized: list[tuple[int, str]] = []
    heavy: list[tuple[int, int, str]] = []
    missing = Counter()

    for p in files:
        text = p.read_text(encoding="utf-8", errors="replace")
        kb = len(text.encode()) / 1024
        n = NUM.match(p.name).group(1).lstrip("0") or "0"
        rel = str(p.relative_to(proj))
        numbers[n].append(rel)
        st = status_of(text)
        fm = frontmatter(text)
        state = str(fm.get("status") or "").lower()
        superseded = bool(fm.get("superseded_by"))
        if not state and SUPERSEDED_BY.search(text):
            state, superseded = "superseded", True

        if not st:
            warns.append(f"{rel}: no Status")
        if superseded and state != "superseded":
            warns.append(f"{rel}: has `superseded_by` but the status is not superseded")

        dec = DECISION.search(text)
        if not dec:
            errors.append(f"{rel}: no '## Decision' section")
        else:
            count = len(ITEM.findall(dec.group(1)))
            if count > args.max_items:
                heavy.append((count, int(kb), rel))

        if kb > args.max_kb:
            oversized.append((int(kb), rel))
        if not WHY.search(text):
            missing["why"] += 1
        if not NOT_DOING.search(text):
            missing["not-doing"] += 1
        if not ENFORCEMENT.search(text):
            missing["enforcement"] += 1
        if state == "superseded" and not superseded:
            errors.append(f"{rel}: superseded but no `superseded_by: [ADR-NNNN]`")

    for n, paths in numbers.items():
        if len(paths) < 2:
            continue
        archived = all("/archived/" in f for f in paths)
        if archived or n in allowed:
            # Grandfathered (ADR-0129 §3 / .duplicates): recorded, never a gate.
            historical.append(f"ADR-{n} shared by {len(paths)} files (historical)")
        else:
            errors.append(f"ADR-{n} used by {len(paths)} files: {', '.join(paths)}")

    oversized.sort(reverse=True)
    heavy.sort(reverse=True)

    print(f"ADRs checked: {len(files)}   budget: {args.max_kb} KB / {args.max_items} items\n")

    if oversized:
        print(f"Over size budget ({len(oversized)}):")
        for kb, f in oversized[:15]:
            print(f"  {kb:>3} KB  {f}")
        if len(oversized) > 15:
            print(f"  … {len(oversized) - 15} more")
        print()

    if heavy:
        print(f"Over decision-count budget ({len(heavy)}):")
        for c, kb, f in heavy[:15]:
            print(f"  {c:>3} items  {kb:>3} KB  {f}")
        print()

    print("Missing the memory fields (new ADRs require them; legacy grandfathered):")
    print(f"  Why / Context          absent in {missing['why']}")
    print(f"  Not doing / rejected   absent in {missing['not-doing']}")
    print(f"  Enforcement            absent in {missing['enforcement']}\n")

    if historical:
        print(f"Historical duplicate numbers ({len(historical)}):")
        for h in historical:
            print(f"  {h}")
        print()

    if warns:
        print(f"Warnings ({len(warns)}):")
        for w in warns[:10]:
            print(f"  {w}")
        if len(warns) > 10:
            print(f"  … {len(warns) - 10} more")
        print()

    if errors:
        print(f"Errors ({len(errors)}):")
        for e in errors:
            print(f"  {e}")
        print()

    bad = bool(errors) or (args.strict and bool(warns))
    bad = bad or (args.strict_budget and bool(oversized or heavy))
    print("FAIL" if bad else "OK")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())

