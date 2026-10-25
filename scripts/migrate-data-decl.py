#!/usr/bin/env python3
"""Rewrite `data X(p : T) :: C | C(args) : R` into the ADR-0158 generator record.

Dry-run by default; --write applies. Line-based: every `data` declaration in the
current tree is a single line (verified: 22 decls / 9 files).
"""
import re, sys, pathlib

DECL = re.compile(r'^data\s+([A-Z][A-Za-z0-9_]*)\s*(?:\(([^)]*)\))?\s*::\s*(.+)$')

def split_top(s, sep):
    """Split on `sep` at depth 0 (parens/brackets/angle-safe)."""
    out, depth, cur = [], 0, ''
    for ch in s:
        if ch in '([<':
            depth += 1
        elif ch in ')]>':
            depth -= 1
        if ch == sep and depth == 0:
            out.append(cur)
            cur = ''
        else:
            cur += ch
    out.append(cur)
    return out

def ctor_entry(ctor, fam):
    """`C` | `C(a, b)` | `C(a) : R` -> (name, record field)"""
    parts = split_top(ctor, ':')
    head = parts[0].strip()
    explicit = ':'.join(parts[1:]).strip() or None
    if '(' in head and head.rstrip().endswith(')'):
        name = head.split('(', 1)[0].strip()
        args = head[head.index('(') + 1 : head.rindex(')')].strip()
    else:
        name, args = head, ''
    if args and explicit:
        return name, f'{name} : ({args}) ~> {explicit}'
    if args:
        return name, f'{name} : ({args}) ~> {fam}'
    if explicit:
        return name, f'{name} : {explicit}'
    return name, f'{name} : {fam}'

def migrate_line(line):
    m = DECL.match(line)
    if not m:
        return None
    fam, params, body = m.group(1), (m.group(2) or '').strip(), m.group(3).strip()
    lhs = f'{fam}<{params}>' if params else fam
    entries = [ctor_entry(c, fam) for c in split_top(body, "|")]
    width = max(len(n) for n, _ in entries)
    rows = [f'  {line_},' for _, line_ in entries]
    if rows:
        rows[-1] = rows[-1][:-1]          # no trailing comma on the last field
    return f'{lhs} := {{\n' + '\n'.join(rows) + '\n}'

files = [pathlib.Path(p) for p in sys.argv[2:]]
write = '--write' in sys.argv
total = 0
for f in files:
    src = f.read_text().splitlines()
    out, changed = [], 0
    for line in src:
        new = migrate_line(line)
        if new is None:
            out.append(line)
        else:
            out.extend(new.splitlines()); changed += 1
    if changed:
        total += changed
        print(f'{f}: {changed} decl(s)')
        if write:
            f.write_text('\n'.join(out) + '\n')
print(f'-- {total} declarations in {sum(1 for f in files)} files' + ('' if not write else ' (written)'))
