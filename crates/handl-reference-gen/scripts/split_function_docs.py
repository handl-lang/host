#!/usr/bin/env python3
"""Split the monolithic docs/function-docs.yaml into per-crate files."""
import os
import re
import sys
from pathlib import Path

import yaml


def build_module_to_crate(crates_dir: Path) -> dict:
    """Map handl module names to their containing crate by scanning source."""
    mod_to_crate = {}
    name_re = re.compile(
        r'fn\s+name\s*\(\s*&self\s*\)\s*->\s*&str\s*\{\s*"([^"]+)"\s*\}'
    )
    for crate_dir in sorted(crates_dir.iterdir()):
        if not crate_dir.is_dir() or not crate_dir.name.startswith('lib-'):
            continue
        src_dir = crate_dir / 'src'
        if not src_dir.is_dir():
            continue
        for rs_file in src_dir.rglob('*.rs'):
            text = rs_file.read_text()
            for m in name_re.finditer(text):
                mod_to_crate[m.group(1)] = crate_dir.name
    return mod_to_crate


def main():
    root = Path(__file__).resolve().parents[3]
    yaml_path = root / 'docs' / 'function-docs.yaml'
    crates_dir = root / 'crates'

    if not yaml_path.exists():
        print(f'{yaml_path} not found; nothing to split.')
        sys.exit(0)

    with open(yaml_path) as f:
        docs = yaml.safe_load(f) or {}

    mod_to_crate = build_module_to_crate(crates_dir)

    per_crate = {}
    for module, functions in docs.items():
        crate = mod_to_crate.get(module)
        if not crate:
            # Functions with no identifiable crate go to a fallback file.
            crate = 'lib-other'
        per_crate.setdefault(crate, {})[module] = functions

    for crate, modules in per_crate.items():
        crate_dir = crates_dir / crate
        crate_dir.mkdir(parents=True, exist_ok=True)
        out_path = crate_dir / 'function-docs.yaml'
        with open(out_path, 'w') as f:
            yaml.safe_dump(
                modules,
                f,
                sort_keys=False,
                allow_unicode=True,
                width=120,
                default_flow_style=False,
            )
        print(f'Wrote {out_path} ({len(modules)} modules)')

    # Remove the monolithic file now that the source of truth is per-crate.
    yaml_path.unlink()
    print(f'Removed {yaml_path}')


if __name__ == '__main__':
    main()
