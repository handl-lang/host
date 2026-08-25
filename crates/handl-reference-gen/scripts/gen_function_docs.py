#!/usr/bin/env python3
"""Generate and merge handl function docs for all lib-* crates."""

import os
import re
from pathlib import Path
import yaml

ROOT = Path('/home/luis/GitHub/handl-lang/platform')
CRATES = ROOT / 'crates'
DOCS = ROOT / 'docs'
OUT = DOCS / 'function-docs.yaml'

TYPE_MAP = {
    'str': 'String',
    'string': 'String',
    'array': 'Array',
    'f64': 'Number',
    'i64': 'Number',
    'number': 'Number',
    'bool': 'Boolean',
    'boolean': 'Boolean',
    'object': 'Object',
    'any': 'type.any',
}

def load_yaml():
    if not OUT.exists():
        return {}
    with open(OUT) as f:
        return yaml.safe_load(f) or {}

def save_yaml(data):
    with open(OUT, 'w') as f:
        yaml.safe_dump(data, f, sort_keys=False, default_flow_style=False, allow_unicode=True)

def extract_doc_comments(content, pos):
    lines = content[:pos].splitlines()
    docs = []
    for line in reversed(lines):
        m = re.match(r'\s*///\s?(.*)', line)
        if m:
            docs.insert(0, m.group(1))
        elif line.strip().startswith('#['):
            continue
        elif line.strip() and not line.strip().startswith('///'):
            break
    return ' '.join(docs).strip()

def extract_call_body(content):
    m = re.search(r'fn\s+call\s*\([^)]*\)\s*->\s*Result<.*>\s*\{', content, re.DOTALL)
    if not m:
        return ''
    start = m.end() - 1
    depth = 0
    for i in range(start, len(content)):
        if content[i] == '{':
            depth += 1
        elif content[i] == '}':
            depth -= 1
            if depth == 0:
                return content[start+1:i]
    return ''

def extract_match_block(content):
    m = re.search(r'fn\s+function\s*\(&self,\s*name:\s*&str\)', content)
    if not m:
        return ''
    m2 = re.search(r'match\s+name\s*\{', content[m.start():])
    if not m2:
        return ''
    start = m.start() + m2.end() - 1
    depth = 0
    for i in range(start, len(content)):
        if content[i] == '{':
            depth += 1
        elif content[i] == '}':
            depth -= 1
            if depth == 0:
                return content[start+1:i]
    return ''

def parse_match_arms(block):
    arms = {}
    for line in block.splitlines():
        line = line.strip()
        m = re.search(r'"([^"]+)"\s*=>\s*Some\(&(?:([a-z][a-z_0-9]*(?:::[a-z][a-z_0-9]*)*|self)\.)?([A-Za-z][A-Za-z0-9_]*)\)', line)
        if m:
            arms[m.group(1)] = m.group(3)
    return arms

def find_function_file(crate_dir, type_name):
    for p in crate_dir.rglob('*.rs'):
        txt = p.read_text()
        if re.search(r'impl\s+(?:<[^>]*>\s*)?Function\s*(?:<[^>]*>)?\s+for\s+' + re.escape(type_name) + r'\b', txt):
            return txt
    return ''

def extract_impl_block(content, type_name):
    pattern = r'impl\s+(?:<[^>]*>\s*)?Function\s*(?:<[^>]*>)?\s+for\s+' + re.escape(type_name) + r'\b\s*\{'
    m = re.search(pattern, content)
    if not m:
        return ''
    start = m.end() - 1
    depth = 0
    for i in range(start, len(content)):
        if content[i] == '{':
            depth += 1
        elif content[i] == '}':
            depth -= 1
            if depth == 0:
                return content[start+1:i]
    return ''

def infer_from_call(call_body, module_doc=''):
    # argument count helpers
    count = 0
    types = []
    # require_string_arg(args, N)
    m = re.search(r'require_string_arg\s*\(\s*args\s*,\s*(\d+)\s*\)', call_body)
    if m:
        count = int(m.group(1))
        types = ['String'] * count
    elif 'require_one_array' in call_body:
        count = 1
        types = ['Array']
    elif 'require_one_number' in call_body:
        count = 1
        types = ['Number']
    elif 'extract_bytes' in call_body:
        count = 1
        types = ['String | array<number>']
    else:
        m = re.search(r'ArgumentCount\s*\{\s*expected:\s*(\d+)', call_body)
        if m:
            count = int(m.group(1))
            types = ['any'] * count
    # merge in explicit args[i].as_* checks (e.g., fs.write validates arg0 and
    # directly checks arg1)
    for i in range(10):
        mm = re.search(rf'args\[{i}\](?:\.\w+)?\.as_(str|array|f64|i64|bool|object)', call_body)
        if mm:
            while len(types) <= i:
                types.append('any')
            types[i] = TYPE_MAP.get(mm.group(1), 'any')
            if i + 1 > count:
                count = i + 1
    if not types:
        types = []

    # return value
    ret = 'type.any'
    if re.search(r'NonFunctionValue::new\s*\(\s*Value::String', call_body) or re.search(r'\bs\s*\(', call_body):
        ret = 'String'
    elif re.search(r'NonFunctionValue::new\s*\(\s*Value::Array', call_body):
        ret = 'Array'
    elif re.search(r'NonFunctionValue::new\s*\(\s*Value::Bool', call_body):
        ret = 'Boolean'
    elif re.search(r'NonFunctionValue::new\s*\(\s*Value::Number', call_body) or re.search(r'\bnum\s*\(', call_body):
        ret = 'Number'
    elif re.search(r'NonFunctionValue::new\s*\(\s*Value::Null', call_body) or re.search(r'NonFunctionValue::default\s*\(\s*\)', call_body):
        ret = 'null'
    elif re.search(r'NonFunctionValue::new\s*\(\s*serde_json::json!\s*\(\s*\{', call_body):
        ret = 'Object'
    elif module_doc and '→' in module_doc:
        m = re.search(r'→\s*([A-Za-z_|\s]+)', module_doc)
        if m:
            ret = m.group(1).strip().lower().replace('number', 'Number').replace('string', 'String')

    return types, ret

def build_signature(module, fn, types):
    if not types:
        return f'{module}.{fn}()'
    args = ', '.join(f'arg{i}: {t}' for i, t in enumerate(types))
    return f'{module}.{fn}({args})'

def build_example(module, fn, types):
    vals = []
    for t in types:
        if 'String' in t:
            vals.append('"x"')
        elif 'Array' in t:
            vals.append('[1, 2, 3]')
        elif 'Number' in t:
            vals.append('1')
        elif 'Boolean' in t:
            vals.append('true')
        else:
            vals.append('"x"')
    return f'{module}.{fn}({", ".join(vals)})'

def extract_examples(doc):
    out = []
    for m in re.finditer(r'`([^`]+)`\s*→', doc):
        s = m.group(1).strip()
        if s:
            out.append(s)
    if not out:
        m = re.search(r'Examples?:\s*(.*?)\n\n', doc, re.S)
        if m:
            for line in m.group(1).splitlines():
                mm = re.search(r'`([^`]+)`', line)
                if mm:
                    out.append(mm.group(1).strip())
    return out

def process_crate(crate_dir, docs):
    for mod_file in crate_dir.rglob('*.rs'):
        content = mod_file.read_text()
        if 'impl Module for' not in content:
            continue
        m = re.search(r'impl\s+Module\s+for\s+([A-Za-z][A-Za-z0-9_]*)', content)
        if not m:
            continue
        mod_struct = m.group(1)
        module_name = None
        m_name = re.search(r'fn\s+name\s*\(&self\)\s*->\s*&str\s*\{\s*"([^"]+)"\s*\}', content)
        if m_name:
            module_name = m_name.group(1)
        else:
            module_name = mod_struct.lower().replace('module', '')

        # module doc from above impl or struct
        mod_doc = extract_doc_comments(content, m.start())

        # struct fields for self.field mapping
        fields = {}
        m_struct = re.search(rf'struct\s+{re.escape(mod_struct)}\s*\{{(.*?)\}}', content, re.S)
        if m_struct:
            for fm in re.finditer(r'(\w+)\s*:\s*([A-Za-z][A-Za-z0-9_]*)', m_struct.group(1)):
                fields[fm.group(1)] = fm.group(2)

        match_block = extract_match_block(content)
        if not match_block:
            continue
        arms = parse_match_arms(match_block)
        if not arms:
            continue

        if module_name not in docs:
            docs[module_name] = {}

        for handl_fn, target in arms.items():
            # resolve target type
            fn_type = target
            if target in fields:
                fn_type = fields[target]
            # search for function impl
            impl_content = find_function_file(crate_dir, fn_type)
            if not impl_content:
                # maybe target is a generic local name; try direct target
                impl_content = find_function_file(crate_dir, target)
            call_body = extract_call_body(extract_impl_block(impl_content, fn_type) or '') if impl_content else ''
            types, ret = infer_from_call(call_body, mod_doc)

            # description: prefer fn/struct docs, then module doc, then generic
            desc = ''
            if impl_content:
                m_impl = re.search(r'impl\s+(?:<[^>]*>\s*)?Function\s*(?:<[^>]*>)?\s+for\s+' + re.escape(fn_type), impl_content)
                if m_impl:
                    impl_pos = impl_content.rfind('\n', 0, m_impl.start()) + 1
                    desc = extract_doc_comments(impl_content, impl_pos)
            if not desc:
                m_struct_fn = re.search(rf'struct\s+{re.escape(fn_type)}', impl_content or '')
                if m_struct_fn:
                    struct_pos = impl_content.rfind('\n', 0, m_struct_fn.start()) + 1
                    desc = extract_doc_comments(impl_content, struct_pos)
            if not desc:
                desc = mod_doc
            if not desc:
                desc = f'Call `{module_name}.{handl_fn}`.'

            # extract example from doc
            examples = extract_examples(desc)
            if not examples:
                examples = [build_example(module_name, handl_fn, types)]

            # first sentence as description, rest as details
            first = re.split(r'\n\n|\.\s+', desc)[0].strip()
            details = desc if first != desc else f'Given {module_name}.{handl_fn}.'

            docs[module_name][handl_fn] = {
                'description': first if first else f'Call `{module_name}.{handl_fn}`.',
                'details': details,
                'signature': build_signature(module_name, handl_fn, types),
                'params': [
                    {
                        'name': f'arg{i}',
                        'type': t,
                        'description': f'the {i}th {t} argument',
                    }
                    for i, t in enumerate(types)
                ],
                'return_value': ret,
                'examples': examples,
                'see': [],
            }

def main():
    docs = load_yaml()
    for crate_dir in sorted(CRATES.glob('lib-*')):
        if crate_dir.name == 'lib-utils':
            # already hand-curated; only add missing functions if desired
            continue
        process_crate(crate_dir, docs)
    save_yaml(docs)
    print(f'Wrote {OUT}')

if __name__ == '__main__':
    main()
