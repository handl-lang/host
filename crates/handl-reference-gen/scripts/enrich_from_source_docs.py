#!/usr/bin/env python3
"""Enrich docs/function-docs.yaml from inline handl doc signatures in lib-* source."""
import re
from pathlib import Path

import yaml

SIGNATURE_RE = re.compile(
    r'`([a-z_][a-z_0-9]*(?:\.[a-z_][a-z_0-9]*)*)\.([a-z_][a-z_0-9_]*)\((.*?)\)(?:\s*→\s*([^\n`]+))?`'
)
ARG_RE = re.compile(r'([a-zA-Z_][a-zA-Z0-9_]*)(\?)?\s*:\s*(.+?)\s*$')
EXAMPLE_RE = re.compile(r'(?:^|\s)-\s*`([^`]+)`(?:\s*→\s*(?:`[^`]*`|"[^"]*"|[^\s`]+))?')

TYPE_MAP = {
    'number': 'Number',
    'num': 'Number',
    'string': 'String',
    'str': 'String',
    'bool': 'Boolean',
    'boolean': 'Boolean',
    'array': 'Array',
    'object': 'Object',
    'value': 'any',
    'any': 'any',
    'null': 'null',
}


def normalize_type(t: str) -> str:
    t = t.strip()
    if t in TYPE_MAP:
        return TYPE_MAP[t]
    if t.endswith('?'):
        base = t[:-1].strip()
        if base in TYPE_MAP:
            return TYPE_MAP[base] + '?'
    return t


def is_stub(entry):
    d = entry.get('description') or ''
    d2 = entry.get('details') or ''
    stubs = ('Call `', 'Given ', 'Invoke the ')
    return d.startswith(stubs) or d == '' or d2.startswith('Given ') or d2 == ''


def extract_doc_blocks(content: str):
    lines = content.splitlines()
    blocks = []
    i = 0
    while i < len(lines):
        s = lines[i].strip()
        if s.startswith('///'):
            block = []
            while i < len(lines) and lines[i].strip().startswith('///'):
                # strip leading /// and optional single space
                block.append(re.sub(r'^///\s?', '', lines[i]).rstrip())
                i += 1
            blocks.append(block)
        else:
            i += 1
    return blocks


def parse_signature(block):
    for line in block:
        m = SIGNATURE_RE.search(line)
        if not m:
            continue
        module_path, fn, args, ret = m.groups()
        # A bare arg list with no types (e.g. "n, locale?, opts?") is a summary
        # list, not a per-function signature.  Allow empty args, but require a
        # colon if any arguments are present.
        if args.strip() and ':' not in args:
            continue
        top = module_path.split('.')[0]
        rest = module_path.split('.')[1:]
        fn_key = '.'.join(rest + [fn]) if rest else fn
        params = []
        for part in args.split(','):
            part = part.strip()
            if not part:
                continue
            am = ARG_RE.match(part)
            if am:
                name, opt, typ = am.groups()
                params.append({
                    'name': name,
                    'type': normalize_type(typ) + ('?' if opt else ''),
                    'description': f'the {name} argument',
                })
            else:
                # No type annotation; treat the part as a bare name.
                bare = part.rstrip('?')
                optional = part.endswith('?')
                params.append({
                    'name': bare,
                    'type': 'any' + ('?' if optional else ''),
                    'description': f'the {bare} argument',
                })
        if ret:
            ret = normalize_type(ret.strip())
        else:
            ret = 'type.any'
        sig = (
            f'{module_path}.{fn}('
            + ', '.join(f"{p['name']}{'?' if p['type'].endswith('?') else ''}: {p['type'].rstrip('?')}"
                       for p in params)
            + ')'
        )
        return {
            'top': top,
            'fn_key': fn_key,
            'module_path': module_path,
            'fn': fn,
            'params': params,
            'return_value': ret,
            'signature': sig,
        }
    return None


def extract_examples(block):
    exs = []
    for line in block:
        for m in EXAMPLE_RE.finditer(line):
            exs.append(m.group(1))
    return exs


def first_sentence(text: str) -> str:
    # Split on paragraph break or period + space
    parts = re.split(r'\n\n|\.\s+', text)
    return parts[0].strip()


def main():
    base = Path(__file__).resolve().parents[3]
    yaml_path = base / 'docs' / 'function-docs.yaml'
    with open(yaml_path) as f:
        docs = yaml.safe_load(f) or {}

    crates_dir = base / 'crates'
    seen = set()
    for p in sorted(crates_dir.glob('lib-*/src/**/*.rs')):
        content = p.read_text()
        for block in extract_doc_blocks(content):
            sig = parse_signature(block)
            if not sig:
                continue
            top = sig['top']
            fn_key = sig['fn_key']
            key = (top, fn_key)
            if key in seen:
                # Prefer the first (most specific) source we find
                continue
            seen.add(key)
            existing = docs.get(top, {}).get(fn_key)
            if existing and not is_stub(existing):
                continue
            full = ' '.join(block)
            description = first_sentence(full)
            details = full if len(block) > 1 else ''
            entry = {
                'description': description,
                'details': details,
                'signature': sig['signature'],
                'params': sig['params'],
                'return_value': sig['return_value'],
                'examples': extract_examples(block),
                'see': [],
            }
            docs.setdefault(top, {})[fn_key] = entry

    with open(yaml_path, 'w') as f:
        yaml.safe_dump(docs, f, sort_keys=False, allow_unicode=True, width=120)
    print(f'Wrote {yaml_path}')


if __name__ == '__main__':
    main()
