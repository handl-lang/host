#!/usr/bin/env python3
"""Split a lib-*.md into per-module pages and create per-module docs.yml files."""
import re
import sys
from pathlib import Path

def main(crate: str):
    root = Path('/home/luis/GitHub/handl-lang/platform')
    crate_dir = root / 'crates' / f'lib-{crate}'
    src = root / 'docs' / 'reference' / f'lib-{crate}.md'

    category_yml = crate_dir / 'docs.yaml'
    if category_yml.exists():
        text = category_yml.read_text()
        m = re.search(r'^\s*slug:\s*(\S+)', text, re.M)
        category_slug = m.group(1) if m else crate
        m = re.search(r'^\s*title:\s*(.+)$', text, re.M)
        category_title = m.group(1) if m else crate.title()
    else:
        category_slug = crate
        category_title = crate.title()

    docs_dir = crate_dir / 'docs'
    docs_dir.mkdir(exist_ok=True)

    lines = src.read_text().splitlines()
    top = []
    modules = {}
    order = []
    current = 'top'
    heading_re = re.compile(r'^## `([a-z][a-z0-9_.]*)`$')

    for line in lines:
        m = heading_re.match(line)
        if m:
            handl = m.group(1)
            if handl.startswith(category_slug + '.'):
                mod = handl[len(category_slug) + 1:]
            else:
                mod = handl
            if mod not in modules:
                modules[mod] = (handl, [])
                order.append(mod)
            modules[mod][1].append(line)
            current = mod
        elif current == 'top':
            top.append(line)
        else:
            modules[current][1].append(line)

    if not order:
        print(f'No module headings found in lib-{crate}.md')
        return

    # create docs.yml for each module
    for mod in order:
        handl, _ = modules[mod]
        title = ' '.join(p.capitalize() for p in mod.split('_'))
        (docs_dir / f'{mod}.yml').write_text(
            f'category:\n'
            f'  slug: lib-{crate}\n'
            f'  title: {category_title}\n'
            f'module:\n'
            f'  name: {handl}\n'
            f'  title: {title}\n'
        )

    # write index
    index = '\n'.join(top).rstrip() + '\n\n'
    index += '## Modules\n\n'
    for mod in order:
        title = ' '.join(p.capitalize() for p in mod.split('_'))
        index += f'- [{title}](./lib-{crate}-{mod})\n'
    index += '\n'
    src.write_text(index)

    # write per-module pages
    heading_pattern = re.compile(r'(## `[^`]+`\n\n)+')
    def collapse_headings(text):
        return heading_pattern.sub(lambda m: m.group(0).split('\n\n')[0] + '\n\n', text)

    for mod in order:
        handl, body_lines = modules[mod]
        title = ' '.join(p.capitalize() for p in mod.split('_'))
        body = '\n'.join(body_lines)
        body = collapse_headings(body)
        md = f'# {title}\n\n{body}'
        (root / 'docs' / 'reference' / f'lib-{crate}-{mod}.md').write_text(md)

    print(f'Split lib-{crate}.md into {len(order)} module pages and created docs.yml files')

if __name__ == '__main__':
    if len(sys.argv) != 2:
        print('usage: split_lib_md.py <crate>  (e.g. collections, utils, os)')
        sys.exit(1)
    main(sys.argv[1])
