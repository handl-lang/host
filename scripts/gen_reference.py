#!/usr/bin/env python3
"""Generate the VitePress reference docs from per-crate docs.yaml and per-module docs/*.yml."""
import json
import re
from pathlib import Path

import yaml

ROOT = Path('/home/luis/GitHub/handl-lang/platform')
REF = ROOT / 'docs' / 'reference'


def esc(text: str) -> str:
    return text.replace('<', '&lt;').replace('>', '&gt;') if text else ''


def title_case(s: str) -> str:
    parts = re.split(r'[-_]', s)
    return ' '.join(p.capitalize() for p in parts if p)


def write_function_doc(md: list[str], doc: dict, full_name: str) -> None:
    md.append(f'## `{full_name}`')
    md.append('')

    sig = doc.get('signature', '')
    if sig:
        md.append(f'**Signature:** `{esc(sig)}`')
        md.append('')

    details = doc.get('details') or doc.get('description', '')
    if details:
        md.append(f'{esc(details)}')
        md.append('')

    params = doc.get('params') or []
    if params:
        md.append('**Parameters:**')
        md.append('')
        for p in params:
            name = p.get('name', '')
            t = p.get('type') or p.get('param_type', '')
            desc = p.get('description', '')
            line = f'- `{name}` (`{esc(t)}`)'
            if desc:
                line += f' — {esc(desc)}'
            md.append(line)
        md.append('')

    ret = doc.get('return_value', '')
    if ret:
        md.append(f'**Returns:** `{esc(ret)}`')
        md.append('')

    examples = doc.get('examples') or []
    if examples:
        md.append('**Examples:**')
        md.append('')
        for ex in examples:
            md.append('```handl')
            md.append(ex)
            md.append('```')
        md.append('')

    see = doc.get('see') or []
    if see:
        links = ', '.join(f'`{esc(s)}`' for s in see)
        md.append(f'**See also:** {links}')
        md.append('')


def load_crate_data(crate_dir: Path):
    short = crate_dir.name.removeprefix('lib-')
    docs_yaml = crate_dir / 'docs.yaml'
    if not docs_yaml.exists():
        return {
            'slug': short,
            'title': title_case(short),
            'description': '',
            'sub_pages': [],
        }
    with open(docs_yaml) as f:
        data = yaml.safe_load(f) or {}
    cat = data.get('category', {})
    if isinstance(cat, str):
        cat = {}
    if not isinstance(cat, dict):
        cat = {}
    return {
        'slug': short,
        'title': cat.get('title') or title_case(short),
        'description': cat.get('description', ''),
        'sub_pages': data.get('sub_pages') or [],
    }


def main():
    REF.mkdir(exist_ok=True)

    # gather crates
    crates = sorted(p for p in (ROOT / 'crates').glob('lib-*') if p.is_dir() and (p / 'docs.yaml').exists())
    categories = [load_crate_data(c) for c in crates]
    slug_to_title = {f'lib-{c["slug"]}': c['title'] for c in categories}

    sub_pages_set = {sub for c in categories for sub in c['sub_pages']}

    # build module pages
    for crate_dir in crates:
        cat = load_crate_data(crate_dir)
        cat_slug = f'lib-{cat["slug"]}'
        modules_dir = crate_dir / 'docs'
        if not modules_dir.is_dir():
            continue

        mod_links = []
        for mod_yml in sorted(modules_dir.glob('*.yml')):
            with open(mod_yml) as f:
                mod_data = yaml.safe_load(f) or {}
            mod = mod_data.get('module', {})
            mod_name = mod.get('name', mod_yml.stem)
            mod_title = mod.get('title') or title_case(mod_yml.stem)
            functions = mod_data.get('functions', {})

            md: list[str] = [f'# {mod_title}', '']
            if mod.get('description'):
                md.append(mod['description'])
                md.append('')

            for fn_key in sorted(functions):
                doc = functions[fn_key]
                full = f'{mod_name}.{fn_key}'
                write_function_doc(md, doc, full)

            page_path = REF / f'{cat_slug}-{mod_yml.stem}.md'
            page_path.write_text('\n'.join(md).rstrip() + '\n')
            mod_links.append((mod_title, f'{cat_slug}-{mod_yml.stem}'))

        # crate index
        index: list[str] = [f'# {cat["title"]}', '']
        if cat['description']:
            index.append(cat['description'])
            index.append('')
        index.append('## Modules')
        index.append('')
        for title, link in mod_links:
            index.append(f'- [{title}](./{link})')
        index.append('')
        (REF / f'{cat_slug}.md').write_text('\n'.join(index))

    # reference.yaml
    reference = []
    for crate_dir, cat in zip(crates, categories):
        reference.append({
            'title': cat['title'],
            'slug': f'lib-{cat["slug"]}',
            'description': cat['description'],
            'include': [{'crate': f'handl-{crate_dir.name}'}],
        })
    (ROOT / 'docs' / 'reference.yaml').write_text(yaml.safe_dump({'reference': reference}, sort_keys=False))

    # sidebar
    sidebar = []
    for crate_dir in crates:
        cat = load_crate_data(crate_dir)
        cat_slug = f'lib-{cat["slug"]}'
        if cat_slug in sub_pages_set:
            continue
        items = []
        for sub in cat['sub_pages']:
            sub_title = slug_to_title.get(sub, sub)
            items.append({'text': sub_title, 'link': f'/reference/{sub}'})

        modules_dir = crate_dir / 'docs'
        if modules_dir.is_dir():
            for mod_yml in sorted(modules_dir.glob('*.yml')):
                with open(mod_yml) as f:
                    mod_data = yaml.safe_load(f) or {}
                mod = mod_data.get('module', {})
                mod_title = mod.get('title') or title_case(mod_yml.stem)
                items.append({'text': mod_title, 'link': f'/reference/{cat_slug}-{mod_yml.stem}'})

        sidebar.append({
            'text': cat['title'],
            'collapsed': False,
            'link': f'/reference/{cat_slug}',
            'items': items,
        })

    (ROOT / 'docs' / '.vitepress' / 'sidebar-reference.mjs').write_text(
        f'export default {json.dumps(sidebar, indent=2)};\n'
    )

    print(f'Generated {len(list(REF.glob("*.md")))} reference pages and sidebar.')


if __name__ == '__main__':
    main()
