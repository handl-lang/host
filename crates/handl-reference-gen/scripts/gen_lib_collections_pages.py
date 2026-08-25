#!/usr/bin/env python3
"""Prototype: split lib-collections.md into per-module pages and create per-module docs.yml."""
import re
from pathlib import Path

ROOT = Path('/home/luis/GitHub/handl-lang/platform')
CRATE = ROOT / 'crates' / 'lib-collections'
DOCS_DIR = CRATE / 'docs'
DOCS_DIR.mkdir(exist_ok=True)

MODULES = {
    'array': 'Array',
    'deque': 'Deque',
    'hash_map': 'Hash Map',
    'hash_set': 'Hash Set',
    'list': 'List',
    'queue': 'Queue',
    'stack': 'Stack',
    'vec': 'Vec',
}

# Create minimal per-module docs.yml files
def module_yml(mod: str, title: str) -> str:
    return (
        f"category:\n"
        f"  slug: lib-collections\n"
        f"  title: Collections\n"
        f"module:\n"
        f"  name: collections.{mod}\n"
        f"  title: {title}\n"
    )

for mod, title in MODULES.items():
    (DOCS_DIR / f'{mod}.yml').write_text(module_yml(mod, title))

src = (ROOT / 'docs' / 'reference' / 'lib-collections.md').read_text()
lines = src.splitlines()

top = []
modules = {m: [] for m in MODULES}
module_order = []
current = 'top'

for line in lines:
    m = re.match(r'^## `collections\.([a-z_]+)`$', line)
    if m:
        mod = m.group(1)
        if mod in modules:
            current = mod
            if mod not in module_order:
                module_order.append(mod)
            modules[mod].append(line)
        else:
            current = 'unknown'
    elif current == 'top':
        top.append(line)
    elif current in modules:
        modules[current].append(line)

# Write the crate index page
index = '\n'.join(top).rstrip() + '\n\n'
index += '## Modules\n\n'
for mod in module_order:
    title = MODULES.get(mod, mod)
    index += f'- [{title}](./lib-collections-{mod})\n'
index += '\n'
(ROOT / 'docs' / 'reference' / 'lib-collections.md').write_text(index)

# Write per-module pages, collapsing duplicate module headings
for mod in module_order:
    title = MODULES.get(mod, mod)
    body = '\n'.join(modules[mod])
    # collapse repeated ## `collections.x` headings into one
    body = re.sub(
        r'(## `collections\.' + re.escape(mod) + r'`\n\n)+',
        f'## `collections.{mod}`\n\n',
        body,
    )
    # The module page should keep the original heading; no need to repeat title
    md = f'# {title}\n\n{body}'
    (ROOT / 'docs' / 'reference' / f'lib-collections-{mod}.md').write_text(md)

print('Generated per-module docs.yml and per-module pages for lib-collections')
