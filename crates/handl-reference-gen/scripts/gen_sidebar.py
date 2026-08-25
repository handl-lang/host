#!/usr/bin/env python3
"""Generate docs/.vitepress/sidebar-reference.mjs from reference.yaml and per-module docs.yml."""
import json
import re
from pathlib import Path

import yaml

ROOT = Path('/home/luis/GitHub/handl-lang/platform')


def title_case(mod: str) -> str:
    return ' '.join(p.capitalize() for p in mod.split('_'))


def main():
    with open(ROOT / 'docs' / 'reference.yaml') as f:
        catalog = yaml.safe_load(f)

    # map every top-level slug to its title
    slug_to_title = {
        cat['slug']: cat.get('title', cat['slug'])
        for cat in catalog.get('reference', [])
    }

    sidebar = []
    for cat in catalog.get('reference', []):
        slug = cat['slug']
        title = cat.get('title', slug)
        krate = cat.get('include', [{}])[0].get('crate', '')
        crate_dir = krate.removeprefix('handl-') if krate else slug.removeprefix('lib-')
        docs_dir = ROOT / 'crates' / crate_dir / 'docs'
        docs_yaml = ROOT / 'crates' / crate_dir / 'docs.yaml'

        items = []

        # sub-pages declared in the crate's docs.yaml
        if docs_yaml.is_file():
            try:
                with open(docs_yaml) as f:
                    crate_data = yaml.safe_load(f) or {}
            except Exception:
                crate_data = {}
            for sub_slug in crate_data.get('sub_pages', []):
                sub_title = slug_to_title.get(sub_slug, sub_slug)
                items.append({
                    'text': sub_title,
                    'link': f'/reference/{sub_slug}',
                })

        # per-module pages from docs/{module}.yml
        if docs_dir.is_dir():
            for yml in sorted(docs_dir.glob('*.yml')):
                try:
                    with open(yml) as f:
                        data = yaml.safe_load(f) or {}
                except Exception:
                    continue
                mod_data = data.get('module', {})
                mod_title = mod_data.get('title') or title_case(yml.stem)
                items.append({
                    'text': mod_title,
                    'link': f'/reference/{slug}-{yml.stem}',
                })

        sidebar.append({
            'text': title,
            'collapsed': False,
            'link': f'/reference/{slug}',
            'items': items,
        })

    js = f'export default {json.dumps(sidebar, indent=2)};\n'
    out = ROOT / 'docs' / '.vitepress' / 'sidebar-reference.mjs'
    out.write_text(js)
    print(f'Wrote {out} ({len(sidebar)} categories)')


if __name__ == '__main__':
    main()
