#!/usr/bin/env python3
"""Validate — and during migration fill — YAML frontmatter on `project/` artifacts.

ADR-0163: every Markdown artifact under `project/` opens with a YAML header
carrying `title`, `type` and `date`; ADRs and RFCs also carry `status`. The
header is the machine-readable form of what used to be filenames
(`notes/2026-08-24-x.md`) and ad-hoc `- Status:` / `**Status**:` / `## Status`.

The schema is closed (eleven keys):

    title           required   H1 without its `ADR-NNNN:` / `RFC-NNNN:` / `Task:` prefix
    type            required   adr design rfc task backlog note report retro plan
                               reference principle goal readme   (index/template exempt)
    date            required   ISO-8601 `YYYY-MM-DD` (or `YYYY-MM` when only the
                               month is known) — creation / decision / measurement
    status          adr, rfc   ADR: proposed accepted implemented landed superseded
                               absorbed rejected unrecorded
                               RFC: draft accepted landed withdrawn
    status_note     optional   the status text: the note behind the enum for ADRs/
                               RFCs; the status itself (verbatim) for other types
    supersedes      optional   [ADR-NNNN] this record replaces
    superseded_by   optional   [ADR-NNNN] the successor (status: superseded)
    absorbed_into   optional   [ADR-NNNN] the absorbing record (status: absorbed)
    refs            optional   [ADR-NNNN] records this document elaborates / relies on
    applies_to      optional   prose scope, when a decision binds more than its repo
    updated         optional   ISO-8601 last substantive edit, living docs only

Usage:
    python3 scripts/project-frontmatter.py                  # check this repo
    python3 scripts/project-frontmatter.py --repo ../graphopt
    python3 scripts/project-frontmatter.py --workspace ..   # every sibling project/
    python3 scripts/project-frontmatter.py --workspace .. --write   # migrate

Exit status: 1 when a tree has errors (missing/invalid frontmatter). Warnings
(`unrecorded` status, title drift, future dates) are reported but pass.
"""

from __future__ import annotations

import argparse
import datetime as dt
import re
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent          # the host repo

DIR_TYPES = {
    "adrs": "adr", "design": "design", "rfcs": "rfc", "tasks": "task",
    "notes": "note", "reports": "report", "retros": "retro", "plans": "plan",
}
FILE_TYPES = {
    "TODO.md": "task", "BACKLOG.md": "backlog", "references.md": "reference",
    "README.md": "readme", "principles.md": "principle", "GOALS.md": "goal",
    "INDEX.md": "index", "DECISIONS.md": "index", "REVIEW_SUMMARY.md": "report",
    "TEMPLATE.md": "template",
}
NO_DATE_TYPES = {"index", "template"}                   # generated / skeleton
ADR_STATUS = ("proposed", "accepted", "implemented", "landed", "superseded",
              "absorbed", "rejected", "unrecorded")
RFC_STATUS = ("draft", "accepted", "landed", "withdrawn")
ORDER = ("title", "type", "date", "status", "status_note", "supersedes",
         "superseded_by", "absorbed_into", "refs", "applies_to", "updated")

FM = re.compile(r"\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*\r?\n?", re.S)
H1 = re.compile(r"^#\s+(.+?)\s*$", re.M)
IDENT = re.compile(r"^(?:ADR[- ]?\d+[a-z]?|RFC[- ]?\d+[a-z]?|Task)\s*[:—–-]\s*", re.I)
DATE_PREFIX = re.compile(r"^(\d{4}-\d{2}(?:-\d{2})?)-")
DATE = re.compile(r"\b(\d{4}-\d{2}(?:-\d{2})?)\b")
STATUS_LINE = re.compile(
    r"^[-*]?\s*(?:\*\*Status\*\*\s*[:—]?|\*\*Status:?\*\*|Status\s*[:—])\s*(.+)$",
    re.M)
DATE_LINE = re.compile(r"^[-*]?\s*\**Date\**\s*[:\u2014]\s*(.+)$", re.M)
REPOS_LINE = re.compile(r"^[-*]\s+Repos\s*:\s*(.+)$", re.M)
HOME = re.compile(r"^##\s+Status\b[^\n]*$(.*?)(?=^##\s|\Z)", re.M | re.S)
ADR_ID = re.compile(r"ADR[-\s]?(\d{3,4})([a-z]?)", re.I)
# Template bullets / pure bold lines — metadata that now lives in frontmatter.
BULLET = re.compile(r"^[-*]\s+(?:Status|Date|Repos|Supersedes)\s*:.*$")
BOLD = re.compile(r"^\*\*(?:Status|Date)\*\*\s*:\s*(.*)$")
BOLD_DATE_LEAD = re.compile(
    r"^\*\*Date\*\*\s*:\s*\d{4}-\d{2}(?:-\d{2})?[.,;]?\s*(?=\*\*)")
PLAIN_DATE = re.compile(r"^Date\s*:\s*\d{4}-\d{2}(?:-\d{2})?[.,;]?\s*$")
ISO = re.compile(r"\d{4}-\d{2}(?:-\d{2})?")
SKIP = {".git", ".gitignore", ".keep", "node_modules", "vendored"}


# ---------------------------------------------------------------- tree walking
def project_tree(repo: Path) -> Path:
    repo = repo.resolve()
    return repo / "project" if (repo / "project").is_dir() else repo


def artifacts(proj: Path) -> list[Path]:
    return [p for p in sorted(proj.rglob("*.md"))
            if not (set(p.relative_to(proj).parts) & SKIP)]


def type_of(rel: Path) -> str | None:
    if rel.name in FILE_TYPES:
        return FILE_TYPES[rel.name]
    for part in rel.parts[:-1]:
        if part in DIR_TYPES:
            return DIR_TYPES[part]
    return None



# -------------------------------------------------------------------- metadata
def split(text: str) -> tuple[dict, str, bool]:
    m = FM.match(text)
    if not m:
        return {}, text, False
    meta = yaml.safe_load(m.group(1)) or {}
    if not isinstance(meta, dict):
        meta = {}
    return meta, text[m.end():], True


def normalize_title(title: str) -> str:
    return re.sub(r"\s+", " ", IDENT.sub("", title)).strip()


def adr_id(text: str) -> list[str]:
    return [f"ADR-{n}{suffix}" for n, suffix in ADR_ID.findall(text)]


def raw_status(body: str) -> tuple[str, bool]:
    """(status as written, came from a one-line field?).

    A `## Status` section wins over a field: it is the record's own statement.
    Either way the prose is moved into `status_note` and deleted from the body
    (ADR-0163) — the body never carries status.
    """
    m = HOME.search(body)
    if m:
        para: list[str] = []
        for line in m.group(1).strip().splitlines():
            if not line.strip():
                if para:
                    break
                continue
            para.append(line.strip())
        return re.sub(r"\s+", " ", " ".join(para)).strip(), False
    m = STATUS_LINE.search(body[:6000])
    if m:
        return m.group(1).strip(" *"), True
    return "", False


def normalize_status(raw: str, kind: str) -> tuple[str, str | None]:
    """(enum value, note). Never invents a status: unknown → `unrecorded`."""
    text = re.sub(r"\s+", " ", raw).strip()
    low = re.sub(r"[*`]", "", text).strip().rstrip(".").lower()
    if kind == "rfc":
        for word in RFC_STATUS:
            if low.startswith(word):
                return word, (text if low != word else None)
        return "draft", (text or None)
    if not low:
        return "unrecorded", None
    if low.startswith("partially implemented"):
        return "accepted", text
    if low.startswith("decided"):
        return ("rejected" if "negative" in low else "accepted"), text
    for word in ("superseded", "absorbed", "rejected", "implemented", "landed",
                 "accepted", "proposed"):
        if low.startswith(word):
            return word, (text if low != word else None)
    if low.startswith("complete"):
        return "implemented", text
    for word in ADR_STATUS:
        if low.startswith(word):
            return word, (text if low != word else None)
    return "unrecorded", text or None


def body_metadata(body: str, kind: str) -> dict:
    meta: dict = {}
    raw, _ = raw_status(body) if kind in ("adr", "rfc") else ("", False)
    if raw:
        status, _note = normalize_status(raw, kind)
        meta["status"] = status
    head = body[:6000]
    m = DATE_LINE.search(head)
    if m and (d := DATE.search(m.group(1))):
        meta["date"] = d.group(1)
    m = REPOS_LINE.search(head)
    if m:
        meta["applies_to"] = re.sub(r"\s+", " ", m.group(1)).strip()
    refs: list[str] = []
    for line in head.splitlines():
        if re.match(r"^[-*]?\s*\**Refs\**\s*[:—]", line, re.I):
            refs += adr_id(line)
    if refs:
        meta["refs"] = list(dict.fromkeys(refs))
    if meta.get("status") == "superseded" and (ids := adr_id(raw)):
        meta["superseded_by"] = ids
        refs = [r for r in refs if r not in ids]
    if meta.get("status") == "absorbed" and (ids := adr_id(raw)):
        meta["absorbed_into"] = ids
        refs = [r for r in refs if r not in ids]
    if refs:
        meta["refs"] = list(dict.fromkeys(refs))
    m = re.search(r"^[-*]?\s*Supersedes\s*:\s*(.+)$", head, re.M | re.I)
    if m and (ids := adr_id(m.group(1))):
        meta["supersedes"] = ids
    return meta


def merge_notes(a: str | None, b: str | None) -> str | None:
    """Keep one copy of the status prose, preferring the fuller statement."""
    a = (a or "").strip()
    b = (b or "").strip()
    if not a:
        return b or None
    if not b or b in a:
        return a
    if a in b:
        return b
    return a + "\n\n" + b


def take_status_prose(body: str) -> tuple[str | None, str, bool]:
    """Move every document-level status statement out of the body.

    Two shapes are status prose, for **every** artifact type:
      * a `## Status` section (heading + content), and
      * a status field in the header block (before the first `##` heading) plus
        its wrapped continuation lines.

    Per-step statuses inside a task's `###` sections are progress, not the
    document's status, and stay. Returns `(note, body, from_section)` — a
    section statement is authoritative over an earlier field note.
    """
    lines = body.splitlines()
    notes: list[str] = []
    from_section = False

    # 1. `## Status` sections — content is the note, every heading goes away.
    while True:
        i = next((k for k, line in enumerate(lines)
                  if re.fullmatch(r"##\s+Status\b[^\n]*", line)), None)
        if i is None:
            break
        from_section = True
        j = i + 1
        content: list[str] = []
        while j < len(lines) and not re.match(r"^##\s", lines[j]):
            content.append(lines[j])
            j += 1
        note = "\n".join(content).strip()
        if note:
            notes.append(note)
        start, end = i, j
        while start > 0 and not lines[start - 1].strip():
            start -= 1
        while end < len(lines) and not lines[end].strip():
            end += 1
        del lines[start:end]
        if 0 < start < len(lines) and lines[start - 1].strip() and lines[start].strip():
            lines.insert(start, "")

    # 2. Header status field (before the first `##`), with wrapped lines.
    cut = next((k for k, ln in enumerate(lines) if re.match(r"^##\s", ln)), len(lines))
    for i in range(cut):
        if not STATUS_LINE.match(lines[i]):
            continue
        take = [i]
        k = i + 1
        while k < cut:
            s = lines[k].strip()
            if (not s or s.startswith("#") or re.match(r"^[-*]\s", s)
                    or re.match(r"^\*\*[^*]+\*\*\s*:", s)
                    or re.match(r"^\*\*[^*]*:\s*\*\*", s)):
                break
            take.append(k)
            k += 1
        first = STATUS_LINE.match(lines[i]).group(1).strip()
        note = "\n".join([first] + [lines[t] for t in take[1:]]).strip()
        if note:
            notes.append(note)
        start, end = take[0], take[-1] + 1
        while start > 0 and not lines[start - 1].strip():
            start -= 1
        while end < len(lines) and not lines[end].strip():
            end += 1
        del lines[start:end]
        if 0 < start < len(lines) and lines[start - 1].strip() and lines[start].strip():
            lines.insert(start, "")
        break

    note = None
    for part in notes:
        note = merge_notes(note, part)
    return note, "\n".join(lines) + "\n", from_section


RELATION = re.compile(r"\b(?:superseded\s+by|supersedes|absorbed\s+into)\b", re.I)


def _sentence_bounds(text: str, start: int) -> tuple[int, int]:
    """(start, end) of the sentence containing `start`, ignoring `.md`/`(…)`."""
    left = max(text.rfind(". ", 0, start), text.rfind("\n\n", 0, start))
    begin = 0 if left < 0 else left + (2 if text[left:left + 2] == "\n\n" else 2)
    depth, code, i = 0, False, start
    while i < len(text):
        c = text[i]
        if c == "`":
            code = not code
        elif not code:
            if c == "(":
                depth += 1
            elif c == ")" and depth:
                depth -= 1
            elif c == "." and depth == 0 and (i + 1 == len(text)
                                              or text[i + 1].isspace()
                                              or text[i + 1] == "*"):
                return begin, i + 1
        i += 1
    return begin, len(text)


def strip_supersession_prose(note: str | None, meta: dict) -> str | None:
    """Drop a `Superseded by ADR-N` sentence — the link is in the header.

    Only when the relation is already structured (`supersedes`,
    `superseded_by`, `absorbed_into`); an unstructured note keeps its prose.
    Rationale that does not name an ADR survives.
    """
    note = (note or "").strip()
    if not note or not any(meta.get(k) for k in
                           ("supersedes", "superseded_by", "absorbed_into")):
        return note or None
    spans: list[tuple[int, int]] = []
    for m in RELATION.finditer(note):
        begin, end = _sentence_bounds(note, m.start())
        sentence = note[begin:end]
        if re.search(r"\bADR\b", sentence):          # only structured references
            spans.append((begin, end))
    if spans:
        out, cursor = [], 0
        for begin, end in sorted(spans):
            if begin < cursor:
                begin = cursor
            out.append(note[cursor:begin])
            cursor = max(cursor, end)
        out.append(note[cursor:])
        note = "".join(out)
    note = re.sub(r"(?m)^\s*\*\*\s*$", "", note)
    while True:                                   # the enum is in `status` already
        m = re.match(r"^\s*(?:proposed|accepted|implemented|landed|rejected|"
                     r"unrecorded|draft)\b[.!]?\s*(?:\n+|$)", note, re.I)
        if not m:
            break
        note = note[m.end():]
    note = re.sub(r"[ \t]+\n", "\n", note)
    note = re.sub(r"\n{3,}", "\n\n", note).strip()
    if not note:
        return None
    status = str(meta.get("status") or "").strip().lower()
    if status and note.lower().strip(" .*") == status:
        return None
    return note


def derive_relations(note: str | None, status: str | None) -> dict:
    """Promote `Supersedes / Superseded by ADR-N` prose into structured links."""
    out: dict[str, list[str]] = {}
    for m in RELATION.finditer(note or ""):
        begin, end = _sentence_bounds(note, m.start())
        sentence = note[begin:end]
        if re.search(r"partially", sentence, re.I):
            continue                              # not representable as one state
        ids = adr_id(sentence)[:1]                # the relation target, not refs
        if not ids:
            continue
        verb = m.group(0).lower()
        if "superseded" in verb and status == "superseded":
            out.setdefault("superseded_by", ids)
        elif "supersedes" in verb:
            out.setdefault("supersedes", ids)
        elif "absorbed" in verb:
            out.setdefault("absorbed_into", ids)
    return out


def git_created(proj: Path, path: Path) -> str | None:
    try:
        out = subprocess.run(
            ["git", "-C", str(proj), "log", "--diff-filter=A", "--follow",
             "--format=%as", "--", str(path.relative_to(proj))],
            capture_output=True, text=True, check=True).stdout.split()
    except subprocess.CalledProcessError:
        return None
    return out[-1] if out else None


def infer_date(meta: dict, body: str, path: Path, proj: Path) -> tuple[str, str]:
    if meta.get("date"):
        return str(meta["date"]), "frontmatter"
    m = DATE_LINE.search(body[:6000])
    if m and (d := DATE.search(m.group(1))):
        return d.group(1), "body"
    if (m := DATE_PREFIX.match(path.name)):
        return m.group(1), "filename"
    if (d := git_created(proj, path)):
        return d, "git"
    return dt.date.fromtimestamp(path.stat().st_mtime).isoformat(), "mtime"



def strip_metadata_lines(body: str, kind: str) -> str:
    """Drop the template bullets / pure `**Status**:` / `**Date**:` lines.

    Compound retro lines (`**Date**: … . **ADR**: …`) keep their other fields,
    so only a line whose whole value is the date is removed.
    """
    out: list[str] = []
    skipping_blank = False
    for line in body.splitlines():
        drop = False
        if kind in ("adr", "rfc") and BULLET.match(line):
            drop = True
        if PLAIN_DATE.match(line):
            drop = True
        if BOLD_DATE_LEAD.match(line):
            line = BOLD_DATE_LEAD.sub("", line)   # keep the other fields
        m = BOLD.match(line)
        if m and not re.search(r"\*\*\w[\w ]*\*\*\s*:", m.group(1)):
            if line.startswith("**Date**"):
                drop = bool(re.fullmatch(
                    r"[*`\s]*\d{4}-\d{2}(?:-\d{2})?[.,;]?\s*", m.group(1)))
            else:
                drop = True
        if not drop and re.fullmatch(r"[-*]\s+Refs\s*:\s*(?:ADR-\d+[a-z]?[,\s]*)+",
                                     line, re.I):
            drop = True                       # pure id list; now in `refs`
        if drop:
            skipping_blank = True
            continue
        if skipping_blank and not line.strip() and out and not out[-1].strip():
            continue
        skipping_blank = False
        out.append(line)
    return "\n".join(out) + ("\n" if body.endswith("\n") else "")


def scalar(value) -> str:
    if isinstance(value, (dt.date, dt.datetime)):
        return value.strftime("%Y-%m-%d")
    if isinstance(value, list):
        return "[" + ", ".join(scalar(v) for v in value) + "]"
    text = str(value)
    if ISO.fullmatch(text):                     # unquoted, conventional
        return text
    dumped = yaml.safe_dump(text, default_flow_style=True,
                            allow_unicode=True, width=4096).strip()
    return dumped[:-4] if dumped.endswith("\n...") else dumped


def render(meta: dict) -> str:
    lines = ["---"]
    keys = [k for k in ORDER if k in meta]
    keys += sorted(k for k in meta if k not in ORDER)
    for key in keys:
        value = meta[key]
        if value in (None, "", []):
            continue
        if (key == "status_note" and isinstance(value, str)
                and ("\n" in value or len(value) > 120)):
            lines.append("status_note: |-")
            for ln in value.splitlines():
                lines.append(("  " + ln).rstrip() if ln.strip() else "")
        else:
            lines.append(f"{key}: {scalar(value)}")
    lines.append("---")
    return "\n".join(lines) + "\n"


def build(meta: dict, body: str, path: Path, kind: str, proj: Path) -> tuple[str, str]:
    date, source = infer_date(meta, body, path, proj)
    found = body_metadata(body, kind)
    found.pop("date", None)                     # provenance stays with infer_date
    note, body, from_section = take_status_prose(body)
    merged = {**found, **{k: v for k, v in meta.items()
                          if v not in (None, "", [])}}
    if from_section and note:
        merged["status_note"] = note          # the section is the record
    else:
        merged["status_note"] = merge_notes(merged.get("status_note"), note)
    for key, ids in derive_relations(merged.get("status_note"),
                                     merged.get("status")).items():
        merged.setdefault(key, ids)
    merged["status_note"] = strip_supersession_prose(
        merged.get("status_note"), merged)
    merged.setdefault("type", kind)
    if head := H1.search(body):
        merged.setdefault("title", normalize_title(head.group(1)))
    if kind in ("adr", "rfc") and not merged.get("status"):
        merged["status"] = "unrecorded"
    merged["date"] = date
    body = strip_metadata_lines(body, kind).lstrip("\n")
    return render(merged) + "\n" + body, source


# ---------------------------------------------------------------------- checks
def check(proj: Path) -> tuple[list[str], list[str], dict]:
    errors: list[str] = []
    warns: list[str] = []
    stats = {"files": 0, "missing": 0, "unrecorded": 0, "no_date": 0}
    today = dt.date.today().isoformat()
    for path in artifacts(proj):
        rel = path.relative_to(proj)
        kind = type_of(rel)
        stats["files"] += 1
        if kind is None:
            errors.append(f"{rel}: unclassified path — no type in ADR-0163")
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        meta, body, has_fm = split(text)
        if kind == "template":
            continue
        if not has_fm:
            errors.append(f"{rel}: no frontmatter")
            stats["missing"] += 1
            continue
        if re.search(r"^##\s+Status\b", body, re.M):
            errors.append(f"{rel}: status section — status lives only in "
                          "frontmatter (ADR-0163)")
        cut = body.find("\n## ")
        header = body if cut < 0 else body[:cut]
        if STATUS_LINE.search(header):
            errors.append(f"{rel}: status line in the body header — status lives "
                          "only in frontmatter (ADR-0163)")
        if kind not in ("adr", "rfc") and meta.get("status"):
            errors.append(f"{rel}: `status` is ADR/RFC-only; use `status_note`")
        if "status_note" in meta and not isinstance(meta["status_note"], str):
            errors.append(f"{rel}: `status_note` must be a string")
        for key in ("title", "type", "date"):
            if key not in meta or str(meta[key]).strip() == "":
                if key == "date" and kind in NO_DATE_TYPES:
                    continue
                errors.append(f"{rel}: missing `{key}`")
        if meta.get("type") != kind:
            errors.append(f"{rel}: type `{meta.get('type')}` != `{kind}` for this path")
        date = meta.get("date")
        if date is not None and not re.fullmatch(r"\d{4}-\d{2}(-\d{2})?", str(date)):
            errors.append(f"{rel}: date `{date}` is not ISO-8601 (YYYY-MM-DD)")
        elif date is not None and str(date) > today:
            warns.append(f"{rel}: date {date} is in the future")
        head = H1.search(body)
        if head and meta.get("title"):
            if normalize_title(head.group(1)) != str(meta["title"]).strip():
                warns.append(f"{rel}: title drifts from the H1")
        if kind == "adr":
            status = meta.get("status")
            if not status:
                errors.append(f"{rel}: ADR has no `status`")
            elif status not in ADR_STATUS:
                errors.append(f"{rel}: status `{status}` not in {', '.join(ADR_STATUS)}")
            elif status == "unrecorded":
                stats["unrecorded"] += 1
                warns.append(f"{rel}: status unrecorded (legacy, triage me)")
            elif status in ("superseded", "absorbed"):
                key = "superseded_by" if status == "superseded" else "absorbed_into"
                if not meta.get(key):
                    errors.append(f"{rel}: status {status} but no `{key}`")
        if kind == "rfc":
            status = meta.get("status")
            if status and status not in RFC_STATUS:
                errors.append(f"{rel}: RFC status `{status}` not in "
                              f"{', '.join(RFC_STATUS)}")
        for key in ("supersedes", "superseded_by", "absorbed_into", "refs"):
            if key in meta and not isinstance(meta[key], list):
                errors.append(f"{rel}: `{key}` must be a list")
        if "date" not in meta:
            stats["no_date"] += 1
    return errors, warns, stats


def migrate(proj: Path, dry_run: bool = False) -> tuple[int, dict]:
    written = 0
    sources: dict[str, int] = {}
    for path in artifacts(proj):
        rel = path.relative_to(proj)
        kind = type_of(rel)
        if kind in (None, "template", "index"):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        meta, body, _ = split(text)
        new, source = build(meta, body, path, kind, proj)
        if new != text:
            sources[source] = sources.get(source, 0) + 1
            written += 1
            if not dry_run:
                path.write_text(new, encoding="utf-8")
    return written, sources


def targets(workspace: Path) -> list[Path]:
    out = []
    for d in sorted(workspace.iterdir()):
        if not d.is_dir() or d.name in SKIP:
            continue
        if (d / "project").is_dir():
            out.append(d)
    if (workspace / "project").is_dir():
        out.append(workspace)
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--repo", type=Path, default=ROOT, help="repo (or project tree)")
    ap.add_argument("--workspace", type=Path, help="walk every sibling repo's project/")
    ap.add_argument("--write", action="store_true", help="fill missing metadata")
    ap.add_argument("--dry-run", action="store_true",
                    help="with --write: report what would change, write nothing")
    args = ap.parse_args(argv)

    repos = targets(args.workspace) if args.workspace else [args.repo]
    failed = False
    for repo in repos:
        proj = project_tree(repo)
        if not proj.is_dir():
            continue
        if args.write:
            n, sources = migrate(proj, dry_run=args.dry_run)
            if n:
                verb = "would write" if args.dry_run else "wrote"
                src = ", ".join(f"{k}={v}" for k, v in sorted(sources.items()))
                print(f"{proj}: {verb} {n} ({src})")
        errors, warns, stats = check(proj)
        status = "FAIL" if errors else "OK"
        print(f"{proj}: {stats['files']} files, "
              f"{len(warns)} warnings, {len(errors)} errors — {status}")
        for e in errors[:20]:
            print(f"  error: {e}")
        if len(errors) > 20:
            print(f"  … {len(errors) - 20} more errors")
        for w in warns[:5]:
            print(f"  warn:  {w}")
        if len(warns) > 5:
            print(f"  … {len(warns) - 5} more warnings")
        failed = failed or bool(errors)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

