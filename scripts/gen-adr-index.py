#!/usr/bin/env python3
"""Generate project/adrs/INDEX.md — the global ADR registry (ADR-0129 §3).

Reads docs-split.yaml for ownership, and each ADR for its title, status and
supersession links. Numbers are globally unique and never renumbered; the
split children of docs-split.yaml `splits:` are listed as `pending`.

Usage: python3 scripts/gen-adr-index.py   # rewrites project/adrs/INDEX.md
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "project"
MANIFEST = PROJECT / "docs-split.yaml"
OUT = PROJECT / "adrs" / "INDEX.md"

NUM = re.compile(r"^(\d+|[0-9]+b?)-")
TITLE = re.compile(r"^#\s+(.*)")
SUPERSEDE = re.compile(
    r"(Superseded by|Supersedes)\s*\[?ADR[-\s]?(\d{3,4})", re.IGNORECASE
)
FRONT = re.compile(r"\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*\r?\n", re.S)


def frontmatter(text: str) -> dict:
    """ADR-0163 header, or {} for a legacy file."""
    m = FRONT.match(text)
    if not m:
        return {}
    try:
        data = yaml.safe_load(m.group(1)) or {}
    except yaml.YAMLError:
        return {}
    return data if isinstance(data, dict) else {}


def read_adr(path: Path) -> dict:
    text = path.read_text(encoding="utf-8", errors="replace")
    fm = frontmatter(text)
    lines = text.splitlines()

    title = str(fm.get("title") or "").strip() or next(
        (TITLE.match(ln).group(1) for ln in lines if TITLE.match(ln)), path.stem)

    status = str(fm.get("status") or "").strip()
    note = str(fm.get("status_note") or "").strip()
    if status and note:
        status = note if note.lower().startswith(status.lower()) else f"{status} — {note}"
    elif note:
        status = note
    for i, ln in enumerate(lines):
        if status:
            break
        if re.match(r"^##\s+Status\s*$", ln):
            rest = [s.strip() for s in lines[i + 1:] if s.strip()]
            status = rest[0] if rest else ""
            break
    if not status:
        m = re.search(r"^\*?\*?Status\*?\*?\s*[:—]\s*(.+)$", text, re.MULTILINE)
        status = m.group(1).strip() if m else ""
    status = status.strip("* ").strip()
    status = re.sub(r"\s+", " ", status).split(".")[0]
    if len(status) > 48:
        status = status[:45].rstrip() + "…"

    links = []
    for verb, num in SUPERSEDE.findall(text[:4000]):
        arrow = "superseded-by" if verb.lower().startswith("superseded") else "supersedes"
        links.append(f"{arrow} {num}")
    for key, arrow in (("supersedes", "supersedes"),
                       ("superseded_by", "superseded-by"),
                       ("absorbed_into", "absorbed-into")):
        for ref in fm.get(key) or []:
            num = re.search(r"(\d{3,4})", str(ref))
            if num:
                links.append(f"{arrow} {num.group(1)}")
    links = list(dict.fromkeys(links))

    date = str(fm.get("date") or "").strip() or "—"

    return {"title": title, "status": status, "date": date, "links": links}


def main() -> int:
    data = yaml.safe_load(MANIFEST.read_text())

    rows = []
    for repo, sections in (data.get("repos") or {}).items():
        for key, names in (sections or {}).items():
            if key not in ("adrs", "adrs_archived"):
                continue
            sub = "adrs" if key == "adrs" else "adrs/archived"
            for name in names or []:
                if not NUM.match(name):
                    continue  # INDEX.md itself, not an ADR
                path = PROJECT / sub / name
                info = read_adr(path) if path.exists() else {
                    "title": name, "status": "missing", "links": []
                }
                num = (NUM.match(name).group(1) if NUM.match(name) else "—")
                rows.append((num, name, repo, sub, info))

    # Union with a directory scan. `docs-split.yaml` is hand-maintained, so
    # a newly written ADR was simply invisible in the registry (0158 and
    # 0159 were both missing while their files were present and committed).
    # Anything on disk that the manifest does not list is added, labelled
    # with this repo — the manifest still owns ordering and split metadata.
    listed = {row[1] for row in rows}
    for sub in ("adrs", "adrs/archived"):
        for path in sorted((PROJECT / sub).glob("*.md")):
            if path.name in listed:
                continue
            match = NUM.match(path.name)
            if not match:
                continue  # INDEX.md and friends
            rows.append(
                (match.group(1), path.name, ROOT.name, sub, read_adr(path))
            )

    rows.sort(key=lambda r: (r[0].rjust(5, "0"), r[1]))

    out = [
        "---",
        "title: ADR index",
        "type: index",
        "---",
        "",
        "# ADR index",
        "",
        "Global registry (ADR-0129 §3). **Numbers are unique and never reused or",
        "renumbered** — allocate the next free number here before writing a new",
        "ADR, even when the file lives in another repo.",
        "",
        "Generated by `scripts/gen-adr-index.py` from `project/docs-split.yaml`.",
        "`status` and `date` come from each ADR's frontmatter (ADR-0163).",
        "",
        "| # | title | status | date | repo | file |",
        "|---|---|---|---|---|---|",
    ]
    for num, name, repo, sub, info in rows:
        link = " ".join(info["links"])
        title = info["title"].replace("|", "\\|")
        status = (info["status"] or "—").replace("|", "\\|")
        if link:
            status += f" ({link})"
        date = (info.get("date") or "—").replace("|", "\\|")
        out.append(f"| {num} | {title} | {status} | {date} | `{repo}` | `{sub}/{name}` |")

    splits = data.get("splits") or []
    if splits:
        out += [
            "",
            "## Split children (ADR-0129 §2)",
            "",
            "Allocated numbers for the non-canonical half of a split ADR. A `—`",
            "number is not yet allocated (the target repo does not exist yet).",
            "",
            "| # | title | repo | file | parent |",
            "|---|---|---|---|---|",
        ]
        for s in splits:
            parent = s["parent"].split("/", 1)[1]
            for t in s.get("targets") or []:
                num, slug = t.get("number"), t.get("slug")
                if not num or not slug:
                    out.append(
                        f"| — | {t['topic']} | `{t['repo']}` | — | `{parent}` |"
                    )
                else:
                    out.append(
                        f"| {num} | {t['title']} | `{t['repo']}` | "
                        f"`adrs/{num}-{slug}.md` | `{parent}` |"
                    )

    locals_ = data.get("local_adrs") or []
    if locals_:
        out += [
            "",
            "## Repo-local ADRs",
            "",
            "Written directly in their repo (not split children of a platform",
            "ADR), listed here so a number is never reused across repos.",
            "",
            "| # | title | repo | file |",
            "|---|---|---|---|",
        ]
        for a in locals_:
            out.append(
                f"| {a['number']} | {a['title']} | `{a['repo']}` | `{a['file']}` |"
            )

    out.append("")
    OUT.write_text("\n".join(out), encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)} — {len(rows)} ADRs, {len(splits)} splits")
    return 0


if __name__ == "__main__":
    sys.exit(main())
