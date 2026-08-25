#!/usr/bin/env python3
"""Split per-crate reference markdown into per-module VitePress pages and sidebar."""
import re
from pathlib import Path

import yaml


def page_name(full: str) -> str:
    parts = full.split('.')
    return parts[-2] if len(parts) >= 2 else parts[-1]


def page_title(name: str) -> str:
    return name.replace('_', ' ').replace('-', ' ').title()


def main():
    base = Path(__file__).resolve().parents[3]
    ref_dir = base / 'docs' / 'reference'
    config_dir = base / 'docs' / '.vitepress'

    catalog_path = base / 'docs' / 'reference.yaml'
    with open(catalog_path) as f:
        catalog = yaml.safe_load(f) or {}

    # Clean up stale per-module pages
    for stale in ref_dir.glob('lib-*-*.md'):
        stale.unlink()

    module_pat = re.compile(r'^## `([^`]+)`\s*\n(.*?)(?=^## `|^### `|\Z)', re.MULTILINE | re.DOTALL)
    func_pat = re.compile(r'^### `([^`]+)`\s*\n(.*?)(?=^### `|^## `|\Z)', re.MULTILINE | re.DOTALL)

    sidebar_groups = []

    for cat in catalog.get('reference', []):
        slug = cat['slug']
        title = cat['title']
        description = cat.get('description') or ''

        md_file = ref_dir / f'{slug}.md'
        if not md_file.exists():
            continue

        text = md_file.read_text()
        module_matches = list(module_pat.finditer(text))

        pages = {}  # page_key -> {module, intro, fns: [(full_name, body)]}

        for i, mm in enumerate(module_matches):
            module = mm.group(1)
            intro = mm.group(2).strip()

            # Ensure the module itself has a page even if it has no functions.
            if module not in pages:
                pages[module] = {'module': module, 'intro': intro, 'fns': []}

            section_start = mm.end()
            section_end = module_matches[i + 1].start() if i + 1 < len(module_matches) else len(text)
            section = text[section_start:section_end]

            for fm in func_pat.finditer(section):
                full_name = fm.group(1)
                body = fm.group(2).strip()
                page = page_name(full_name)

                if page not in pages:
                    # Only attach the module intro to pages whose key equals the module name.
                    use_intro = intro if page == module else ''
                    pages[page] = {'module': module, 'intro': use_intro, 'fns': []}

                pages[page]['fns'].append((full_name, body))

        # Sort pages by first appearance
        seen_pages = []
        for fm in func_pat.finditer(text):
            page = page_name(fm.group(1))
            if page not in seen_pages:
                seen_pages.append(page)

        # Also create pages for modules that have no functions (e.g. serializers).
        for mm in module_matches:
            module = mm.group(1)
            if module not in seen_pages:
                seen_pages.append(module)

        module_links = []
        for page in seen_pages:
            data = pages[page]
            file_slug = f'{slug}-{page}'
            file_path = ref_dir / f'{file_slug}.md'

            lines = [f'# {page_title(page)}', '']
            if data['intro']:
                lines.append(data['intro'])
                lines.append('')

            for full_name, body in data['fns']:
                lines.append(f'## `{full_name}`')
                lines.append('')
                lines.append(body)
                lines.append('')

            file_path.write_text('\n'.join(lines).strip() + '\n')
            module_links.append({'text': page_title(page), 'link': f'/reference/{file_slug}'})

        if not module_links:
            # Crate has no handl-facing modules; write a minimal page from the
            # catalog description and make the section clickable.
            body = [f'# {title}', '']
            if description:
                body.append(description)
                body.append('')
            body.append('This crate is internal to the handl runtime and is not directly exposed to handl scripts as a module or function. It may provide types or traits used by other handl-facing libraries.')
            md_file.write_text('\n'.join(body) + '\n')
            sidebar_groups.append({
                'text': title,
                'collapsed': False,
                'link': f'/reference/{slug}',
                'items': [],
            })
        else:
            # Rewrite the crate overview to a modules index
            overview_lines = [f'# {title}', '']
            if description:
                overview_lines.append(description)
                overview_lines.append('')
            overview_lines.append('## Modules')
            overview_lines.append('')
            for link in module_links:
                overview_lines.append(f"- [{link['text']}]({link['link']})")
            md_file.write_text('\n'.join(overview_lines) + '\n')

            if len(module_links) == 1:
                sidebar_groups.append({
                    'text': title,
                    'collapsed': False,
                    'link': module_links[0]['link'],
                    'items': [],
                })
            else:
                sidebar_groups.append({
                    'text': title,
                    'collapsed': False,
                    'link': f'/reference/{slug}',
                    'items': module_links,
                })

    # Write the generated sidebar for config.mjs to import
    sidebar_mjs = config_dir / 'sidebar-reference.mjs'
    import json
    with open(sidebar_mjs, 'w') as f:
        f.write('export default ')
        json.dump(sidebar_groups, f, indent=2)
        f.write(';\n')

    # Update VitePress config to use the generated sidebar
    config_mjs = config_dir / 'config.mjs'
    config_text = config_mjs.read_text()
    if 'referenceSidebar' not in config_text:
        lines = config_text.split('\n', 1)
        config_text = lines[0] + '\nimport referenceSidebar from "./sidebar-reference.mjs";\n' + lines[1]

    idx = config_text.find('          text: "Reference",\n          link: "/reference/",\n          collapsed: false,')
    if idx != -1:
        start = config_text.find('items: [', idx)
        if start != -1:
            balance = 0
            in_string = False
            esc = False
            end = start
            for i in range(start, len(config_text)):
                c = config_text[i]
                if esc:
                    esc = False
                    continue
                if c == '\\':
                    esc = True
                    continue
                if c == '"' and not in_string:
                    in_string = True
                elif c == '"' and in_string:
                    in_string = False
                if not in_string:
                    if c == '[':
                        balance += 1
                    elif c == ']':
                        balance -= 1
                        if balance == 0:
                            end = i + 1
                            break
            config_text = config_text[:start] + 'items: referenceSidebar,' + config_text[end:]
    config_mjs.write_text(config_text)

    print(f'Wrote {len(sidebar_groups)} reference groups and {len(list(ref_dir.glob("lib-*-*.md")))} module pages.')


if __name__ == '__main__':
    main()
