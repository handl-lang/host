#!/usr/bin/env python3
"""Move per-crate function-docs.yaml into per-module docs/{module}.yml."""
import re
from pathlib import Path

import yaml

ROOT = Path('/home/luis/GitHub/handl-lang/platform')

def to_title_case(s: str) -> str:
    return ' '.join(p.capitalize() for p in s.split('_'))


def normalize_crate_name(s: str) -> str:
    return s.replace('-', '_')


def main():
    for fd in sorted(ROOT.glob('crates/lib-*/function-docs.yaml')):
        crate_dir = fd.parent.name
        with open(fd) as f:
            data = yaml.safe_load(f) or {}

        docs_yaml = fd.parent / 'docs.yaml'
        if docs_yaml.exists():
            with open(docs_yaml) as f:
                crate_meta = yaml.safe_load(f) or {}
            if not isinstance(crate_meta, dict):
                crate_meta = {}
            cat = crate_meta.get('category') or {}
            if isinstance(cat, str):
                cat = {'slug': cat}
            if not isinstance(cat, dict):
                cat = {}
            cat_slug = cat.get('slug', crate_dir.removeprefix('lib-'))
            cat_title = cat.get('title', to_title_case(cat_slug))
        else:
            short = crate_dir.removeprefix('lib-')
            cat_slug, cat_title = short, to_title_case(short)

        for top_key, functions in data.items():
            if not isinstance(functions, dict):
                continue

            grouped: dict[str, dict] = {}
            for fn_key, doc in functions.items():
                if '.' in fn_key:
                    mod_short, fn_short = fn_key.split('.', 1)
                else:
                    mod_short, fn_short = top_key, fn_key
                grouped.setdefault(mod_short, {})[fn_short] = doc

            for mod_short, fns in grouped.items():
                mod_yml = fd.parent / 'docs' / f'{mod_short}.yml'
                mod_yml.parent.mkdir(exist_ok=True)

                if mod_yml.exists():
                    with open(mod_yml) as f:
                        mod_data = yaml.safe_load(f) or {}
                else:
                    mod_data = {}

                # category / module can come from existing docs.yml or be inferred
                mod_data.setdefault('category', {})
                mod_data['category'].setdefault('slug', cat_slug)
                mod_data['category'].setdefault('title', cat_title)

                mod_name = mod_data.get('module', {}).get('name')
                if not mod_name:
                    if cat_slug == mod_short:
                        mod_name = mod_short
                    else:
                        mod_name = f'{cat_slug}.{mod_short}' if mod_short != cat_slug else mod_short

                mod_data.setdefault('module', {})
                mod_data['module'].setdefault('name', mod_name)
                mod_data['module'].setdefault('title', to_title_case(mod_short))

                mod_data.setdefault('functions', {})
                mod_data['functions'].update(fns)

                with open(mod_yml, 'w') as f:
                    yaml.safe_dump(mod_data, f, sort_keys=False)

        print(f'Migrated {fd}')


if __name__ == '__main__':
    main()
