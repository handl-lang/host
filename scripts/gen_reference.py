#!/usr/bin/env python3
"""Generate the VitePress reference docs from handl doc comments.

Regex-based stopgap: the doc comments are *lexed*, not parsed. A proper
extractor will run over the handl parser's AST later; until then this keeps
`docs/reference.yaml` and the sidebar in sync with `handl-packages/`.

Sources are the `---` doc comments in `handl-packages/*/*.hnl`. Three
declaration forms are covered:

    --- math.abs -- absolute value of a number.      (file-level doc block)
    op "math.abs" : Float -> Float

    --- Applies a transform to a value unless ...
    --- @details ...
    --- @example ...
    export unless :: [value, pred, fn] ~> ...

    -- LAW: concat(graphemes(s)) = s
    axiom unicode.graphemes : String -> List String

A declared name that is already dotted (`math.abs`, `base64.encode`,
`unicode.graphemes`) supplies its own module; a bare `export Name` falls back
to the file layout — a single-export file named after that export is a
one-arrow-per-file package (module = package), anything else is a
module-per-file package (module = file stem).

Outputs: `docs/reference/lib-*.md` pages, `docs/reference.yaml`, and
`docs/.vitepress/sidebar-reference.mjs`. Test files and `generated/` are
skipped.
"""
import json
import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
PKGS = ROOT / 'handl-packages'
REF = ROOT / 'docs' / 'reference'

# --- line patterns ----------------------------------------------------------

DOC_LINE = re.compile(r'^---\s?(.*)$')
LAW_LINE = re.compile(r'^--\s?(.*)$')
NAMED_SUMMARY = re.compile(r'^(?P<name>[\w.]+)\s+--\s+(?P<summary>.+)$')
OP_DECL = re.compile(r'^op\s+"(?P<name>[\w.]+)"\s*:\s*(?P<sig>.*)$')
AXIOM_DECL = re.compile(r'^axiom\s+(?P<name>[\w.]+)\s*:\s*(?P<sig>.*)$')
EXPORT_DECL = re.compile(r'^export\s+(?P<name>[A-Za-z_]\w*)(?P<rest>.*)$')
REEXPORT_IMPORT = re.compile(r'^from\s+"(?P<path>[^"]+)"\s+import\s+(?P<orig>[\w.]+)\s+as\s+(?P<alias>\w+)$')


def title_case(slug: str) -> str:
    parts = re.split(r'[-_]', slug)
    return ' '.join(p.capitalize() for p in parts if p)


def norm(name: str) -> str:
    return name.lower().replace('_', '')


def slugify(pkg_dir: str) -> str:
    return pkg_dir.replace('_', '-')


def esc(text: str) -> str:
    return str(text).replace('<', '&lt;').replace('>', '&gt;')


def parse_doc(lines: list[str]) -> dict:
    """Split a `---` doc block into summary/details/returns/examples.

    Untagged lines before the first `@tag` are the summary (first line) and
    details (the rest). Recognised tags: `@details`, `@returns`, `@example`.
    """
    body, tag, buckets = [], None, {'details': [], 'returns': [], 'example': []}
    for line in lines:
        if line.startswith('@details') or line.startswith('@returns') or line.startswith('@example'):
            tag = line[1:].split()[0].rstrip(':')
            continue
        (buckets[tag] if tag else body).append(line)

    body = [b for b in body if b.strip()]
    summary = body[0].strip() if body else ''
    # A file-level block often leads with `<name> -- <summary>`; keep just the
    # summary so the heading does not repeat the full name.
    named = NAMED_SUMMARY.match(summary)
    if named:
        summary = named.group('summary').strip()
    details = ' '.join(b.strip() for b in body[1:]).strip()
    return {
        'summary': summary,
        'details': details,
        'returns': ' '.join(b.strip() for b in buckets['returns']).strip(),
        'examples': [e.strip() for e in buckets['example'] if e.strip()],
    }


def doc_block_above(lines: list[str], i: int, pattern) -> list[str]:
    """Contiguous doc-comment lines directly above line `i`, in order."""
    out = []
    j = i - 1
    while j >= 0:
        m = pattern.match(lines[j])
        if not m:
            break
        out.append(m.group(1))
        j -= 1
    return list(reversed(out))


def split_export(rest: str) -> tuple[str, str | None]:
    """`(signature, re-export alias)` for an `export <name> ...` tail.

    `export abs := __abs` re-exports a binding: the RHS is a name, not a
    type, so it has no signature — the alias is returned so the caller can
    point at the import it came from.
    """
    body = re.sub(r'^\s*::?=?\s*', '', rest).strip()
    if re.fullmatch(r'[A-Za-z_]\w*', body):
        return '', body
    for sep in (' ~>', ' :=', ' ::'):
        if sep in body:
            body = body.split(sep)[0].strip()
    return body, None


def module_of(pkg: str, stem: str, name: str, exports_in_file: list[str]) -> str:
    if '.' in name:
        return name.rsplit('.', 1)[0]
    # `index.hnl` is the package facade (`from "../x" import ... ; export x`),
    # not a module of its own.
    if stem == 'index':
        return pkg
    if len(exports_in_file) == 1 and norm(name) == norm(stem):
        return pkg
    return stem


def scan_package(pkg_dir: Path) -> dict | None:
    """Collect documented declarations of one package, grouped by module."""
    pkg = pkg_dir.name
    modules: dict[str, list[dict]] = {}
    description = ''

    files = sorted(
        f for f in pkg_dir.glob('*.hnl') if not f.name.endswith('.test.hnl')
    )
    for path in files:
        lines = path.read_text().splitlines()
        # The file's leading `---` block (index 0 upward).
        header = []
        for line in lines:
            m = DOC_LINE.match(line)
            if not m:
                break
            header.append(m.group(1))
        header_doc = parse_doc(header) if header else {'summary': '', 'details': '', 'returns': '', 'examples': []}
        named = {}
        for line in header:
            m = NAMED_SUMMARY.match(line.strip())
            if m:
                named[m.group('name')] = m.group('summary').strip()
        if not description and header_doc['summary']:
            description = header_doc['summary']

        exports = [EXPORT_DECL.match(l).group('name') for l in lines if EXPORT_DECL.match(l)]
        # `export abs := __abs` is a facade re-export; the import line above
        # says where it comes from, which is the only documentation it has.
        origins = {
            m.group('alias'): (m.group('path'), m.group('orig'))
            for m in (REEXPORT_IMPORT.match(l) for l in lines)
            if m
        }
        decl_count = sum(
            1 for l in lines if OP_DECL.match(l) or AXIOM_DECL.match(l) or EXPORT_DECL.match(l)
        )
        stem = path.stem

        for i, line in enumerate(lines):
            entry = None
            for decl, kind in ((OP_DECL, 'op'), (AXIOM_DECL, 'axiom'), (EXPORT_DECL, 'export')):
                m = decl.match(line)
                if not m:
                    continue
                name = m.group('name')
                alias = None
                if kind == 'export':
                    sig, alias = split_export(m.group('rest'))
                else:
                    sig = m.group('sig').strip()
                block = doc_block_above(lines, i, DOC_LINE)
                if kind == 'axiom':
                    # A contract axiom carries its body *below* as `-- LAW:`
                    # comments (with an optional `guarantee:` line).
                    below, j = [], i + 1
                    while j < len(lines):
                        m = LAW_LINE.match(lines[j])
                        if not m:
                            break
                        below.append(m.group(1).strip())
                        j += 1
                    doc = {
                        'summary': named.get(name, ''),
                        'details': ' '.join(b for b in below if b),
                        'returns': '',
                        'examples': [],
                    }
                elif block:
                    doc = parse_doc(block)
                else:
                    # A file-level block documents the whole file: it is the
                    # doc of a one-declaration file, but a facade's header
                    # (`--- math -- numeric surface facade.`) must not become
                    # every re-export's summary.
                    fallback = named.get(name, header_doc['summary'] if decl_count == 1 else '')
                    doc = {'summary': fallback, 'details': '', 'returns': '', 'examples': []}
                origin = origins.get(alias) if alias else None
                entry = {'name': name, 'sig': sig, 'kind': kind, 'origin': origin, **doc}
                break
            if not entry:
                continue
            module = module_of(pkg, stem, entry['name'], exports)
            leaf = entry['name'].rsplit('.', 1)[-1]
            entry['full'] = f'{module}.{leaf}'
            modules.setdefault(module, []).append(entry)

    modules = {m: sorted(v, key=lambda e: e['name'].lower()) for m, v in modules.items() if v}
    if not modules:
        return None
    return {'pkg': pkg, 'slug': slugify(pkg), 'description': description, 'modules': modules}


def write_module_page(out: Path, module: str, entries: list[dict]) -> None:
    md = [f'# {title_case(module)}', '']
    for e in entries:
        md.append(f'## `{esc(e["full"])}`')
        md.append('')
        if e['summary']:
            md.append(esc(e['summary']))
            md.append('')
        if e['details']:
            md.append(esc(e['details']))
            md.append('')
        if e['origin']:
            path, orig = e['origin']
            md.append(f'Re-export of `{esc(orig)}` from `{esc(path)}`.')
            md.append('')
        elif e['sig']:
            # `[f, args]` / `s` are parameter patterns; `A -> B` / `A ~[n]~> B`
            # are type signatures.
            if '->' in e['sig']:
                md.append(f'**Signature:** `{esc(e["full"])} : {esc(e["sig"])}`')
            else:
                md.append(f'**Signature:** `{esc(e["full"])}({esc(e["sig"])})`')
            md.append('')
        if e['returns']:
            md.append(f'**Returns:** {esc(e["returns"])}')
            md.append('')
        if e['examples']:
            md.append('**Examples:**')
            md.append('')
            md.append('```handl')
            md.extend(e['examples'])
            md.append('```')
            md.append('')
    out.write_text('\n'.join(md).rstrip() + '\n')


def main() -> int:
    REF.mkdir(exist_ok=True)

    dirs = [d for d in sorted(PKGS.iterdir()) if d.is_dir() and d.name != 'generated']
    packages = [p for p in (scan_package(d) for d in dirs) if p]
    # Guard: never overwrite the hand-maintained index with an empty one.
    if not packages:
        print('gen_reference: no documented declarations found in handl-packages/*/*.hnl')
        print('  refusing to overwrite docs/reference.yaml with an empty index.')
        return 0

    written = set()
    reference, sidebar = [], []
    for pkg in packages:
        mod_links = []
        for module, entries in pkg['modules'].items():
            page = f'lib-{pkg["slug"]}-{module}'
            write_module_page(REF / f'{page}.md', module, entries)
            written.add(f'{page}.md')
            mod_links.append((title_case(module), page))

        index = [f'# {title_case(pkg["slug"])}', '']
        if pkg['description']:
            index += [esc(pkg['description']), '']
        index += ['## Modules', '']
        index += [f'- [{t}](./{p})' for t, p in mod_links]
        (REF / f'lib-{pkg["slug"]}.md').write_text('\n'.join(index).rstrip() + '\n')
        written.add(f'lib-{pkg["slug"]}.md')

        reference.append({
            'title': title_case(pkg['slug']),
            'slug': f'lib-{pkg["slug"]}',
            'description': pkg['description'],
            'include': [{'package': pkg['pkg']}],
        })
        sidebar.append({
            'text': title_case(pkg['slug']),
            'collapsed': False,
            'link': f'/reference/lib-{pkg["slug"]}',
            'items': [
                {'text': t, 'link': f'/reference/{p}'} for t, p in mod_links
            ],
        })

    # Drop pages this run did not produce — `lib-*.md` is generator output;
    # hand-written pages in the same dir (index.md, host-concepts.md, …) are
    # left alone.
    for stale in REF.glob('lib-*.md'):
        if stale.name not in written:
            stale.unlink()

    (ROOT / 'docs' / 'reference.yaml').write_text(
        yaml.safe_dump({'reference': reference}, sort_keys=False)
    )
    (ROOT / 'docs' / '.vitepress' / 'sidebar-reference.mjs').write_text(
        f'export default {json.dumps(sidebar, indent=2)};\n'
    )
    print(f'Generated {len(packages)} packages, {len(written)} reference pages, sidebar.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
