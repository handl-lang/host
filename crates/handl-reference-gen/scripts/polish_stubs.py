#!/usr/bin/env python3
"""Generate human-readable descriptions for stub entries in function-docs.yaml."""
import re
from pathlib import Path

import yaml


def is_stub(entry):
    d = entry.get('description') or ''
    d2 = entry.get('details') or ''
    return d.startswith(('Call `', 'Given ', 'Invoke the ')) or d == '' or d2.startswith('Given ')


def p_name(entry, i):
    params = entry.get('params') or []
    if i < len(params):
        return params[i]['name']
    return None


def p_type(entry, i):
    params = entry.get('params') or []
    if i < len(params):
        return params[i]['type'].rstrip('?')
    return 'any'


def base_fn(fn):
    return fn.split('.')[-1]


def sub_module(top, fn):
    if '.' in fn:
        return fn.rsplit('.', 1)[0]
    return top


def full_module(top, fn):
    if '.' in fn:
        return f"{top}.{fn.rsplit('.', 1)[0]}"
    return top


def desc_for(top, fn, ret, entry):
    b = base_fn(fn)
    sm = sub_module(top, fn)
    fm = full_module(top, fn)
    s = signature_vars

    # collections
    if b == 'new' and not s[0]:
        return f"Create and return a new empty `{sm}`.", f"Return a new, empty `{sm}` of type `{fm}`."
    if b == 'from' and s[0]:
        return f"Create a `{sm}` from `{s[0]}`.", f"Convert `{s[0]}` into a new `{sm}` and return it."
    if b in ('to_object', 'to_array') and s[0]:
        target = 'object' if 'object' in b else 'array'
        return f"Convert `{s[0]}` to a plain {target}.", f"Return a {target} containing the same contents as `{s[0]}`."
    if b == 'len' and s[0]:
        return f"Return the number of elements in `{s[0]}`.", f"Return the length of `{s[0]}` as a number."
    if b == 'is_empty' and s[0]:
        return f"Return `true` if `{s[0]}` is empty.", f"Return a boolean indicating whether `{s[0]}` has no elements."
    if b == 'get' and s[0]:
        if s[1]:
            return f"Return the value associated with `{s[1]}` in `{s[0]}`.", f"Look up `{s[1]}` in `{s[0]}` and return the corresponding value."
        return f"Return a value from `{s[0]}`.", f"Retrieve a value from `{s[0]}`."
    if b == 'set' and s[0] and s[1]:
        return f"Set the value for `{s[1]}` in `{s[0]}`.", f"Associate `{s[2] or 'value'}` with key `{s[1]}` in `{s[0]}` and return the updated `{sm}`."
    if b == 'has' and s[0]:
        if s[1]:
            return f"Return `true` if `{s[0]}` contains `{s[1]}`.", f"Check whether `{s[1]}` exists in `{s[0]}`."
        return f"Return `true` if `{s[0]}` has a value.", f"Return whether `{s[0]}` contains the requested value."
    if b == 'remove' and s[0]:
        if s[1]:
            return f"Remove `{s[1]}` from `{s[0]}`.", f"Remove the element or key `{s[1]}` from `{s[0]}` and return the updated `{sm}`."
        return f"Remove `{s[0]}`.", f"Remove `{s[0]}`."
    if b == 'keys' and s[0]:
        return f"Return the keys of `{s[0]}` as an array.", f"Return an array of all keys in `{s[0]}`."
    if b == 'values' and s[0]:
        return f"Return the values of `{s[0]}` as an array.", f"Return an array of all values in `{s[0]}`."
    if b in ('push', 'push_back') and s[0]:
        return f"Append `{s[1] or 'value'}` to `{s[0]}`.", f"Add `{s[1] or 'value'}` to the end of `{s[0]}` and return the updated `{sm}`."
    if b == 'push_front' and s[0]:
        return f"Prepend `{s[1] or 'value'}` to `{s[0]}`.", f"Add `{s[1] or 'value'}` to the front of `{s[0]}` and return the updated `{sm}`."
    if b in ('pop', 'pop_back') and s[0]:
        return f"Remove and return the last element of `{s[0]}`.", f"Remove the last element from `{s[0]}` and return it."
    if b == 'pop_front' and s[0]:
        return f"Remove and return the first element of `{s[0]}`.", f"Remove the first element from `{s[0]}` and return it."
    if b in ('peek', 'peek_back') and s[0]:
        return f"Return the last element of `{s[0]}` without removing it.", f"Inspect the last element of `{s[0]}` without modifying the collection."
    if b == 'peek_front' and s[0]:
        return f"Return the first element of `{s[0]}` without removing it.", f"Inspect the first element of `{s[0]}` without modifying the collection."
    if b == 'enqueue' and s[0]:
        return f"Add `{s[1] or 'value'}` to the back of `{s[0]}`.", f"Enqueue `{s[1] or 'value'}` in `{s[0]}`."
    if b == 'dequeue' and s[0]:
        return f"Remove and return the front element of `{s[0]}`.", f"Dequeue the next element from `{s[0]}`."
    if b == 'chunk' and s[0]:
        return f"Split `{s[0]}` into chunks of size `{s[1] or 'n'}`.", f"Group the elements of `{s[0]}` into arrays of length `{s[1] or 'n'}` (the last chunk may be shorter)."
    if b == 'concat' and s[0]:
        return f"Concatenate the given arrays.", f"Return a new array containing all elements of the input arrays in order."
    if b == 'at' and s[0]:
        return f"Return the element at index `{s[1] or 'i'}` in `{s[0]}`.", f"Access the element at position `{s[1] or 'i'}` in `{s[0]}`."
    if b == 'first' and s[0]:
        return f"Return the first element of `{s[0]}`.", f"Return the first element of `{s[0]}`."
    if b == 'last' and s[0]:
        return f"Return the last element of `{s[0]}`.", f"Return the last element of `{s[0]}`."
    if b == 'reverse' and s[0]:
        return f"Return `{s[0]}` with its elements in reverse order.", f"Return a new collection with the elements of `{s[0]}` reversed."
    if b == 'sort' and s[0]:
        return f"Return `{s[0]}` sorted.", f"Return a new collection containing the elements of `{s[0]}` sorted."
    if b == 'clear' and s[0]:
        return f"Remove all elements from `{s[0]}`.", f"Clear `{s[0]}` and return the empty collection."
    if b == 'insert' and s[0]:
        return f"Insert `{s[1] or 'value'}` into `{s[0]}`.", f"Insert the value `{s[1] or 'value'}` into `{s[0]}` at the appropriate position."
    if b == 'contains' and s[0]:
        return f"Return `true` if `{s[0]}` contains `{s[1] or 'value'}`.", f"Check whether `{s[0]}` contains `{s[1] or 'value'}`."
    if b == 'index_of' and s[0]:
        return f"Return the index of `{s[1] or 'value'}` in `{s[0]}`.", f"Find the position of `{s[1] or 'value'}` within `{s[0]}`."

    # fs / os
    if b == 'read' and top == 'fs' and s[0]:
        return f"Read the contents of the file at `{s[0]}`.", f"Open the file at `{s[0]}`, read its contents, and return them as a string."
    if b == 'write' and top == 'fs' and s[0]:
        return f"Write `{s[1] or 'content'}` to the file at `{s[0]}`.", f"Create or overwrite the file at `{s[0]}` with the string `{s[1] or 'content'}`."
    if b == 'exists' and top == 'fs' and s[0]:
        return f"Return `true` if the file at `{s[0]}` exists.", f"Check whether a file or directory exists at `{s[0]}`."
    if b == 'remove' and top == 'fs' and s[0]:
        return f"Remove the file or directory at `{s[0]}`.", f"Delete the file or directory at `{s[0]}`."
    if b == 'list' and top == 'fs' and s[0]:
        return f"Return the entries in the directory at `{s[0]}`.", f"Return an array of file and directory names inside `{s[0]}`."

    # env
    if b == 'get' and top == 'env' and s[0]:
        return f"Return the value of the environment variable `{s[0]}`.", f"Return the value of the environment variable named `{s[0]}`."
    if b == 'has' and top == 'env' and s[0]:
        return f"Return `true` if the environment variable `{s[0]}` is set.", f"Check whether the environment variable `{s[0]}` exists."
    if b == 'keys' and top == 'env':
        return f"Return an array of all environment variable names.", f"Return the names of all available environment variables."

    # path
    if b == 'join' and top == 'path' and s[0]:
        return f"Join path segments into a single path.", f"Combine the given path segments `{s[0]}`, `{s[1] or ''}`, ... into one path."
    if b == 'resolve' and top == 'path' and s[0]:
        return f"Resolve `{s[0]}` to an absolute path.", f"Return the absolute path for `{s[0]}`."
    if b == 'basename' and top == 'path' and s[0]:
        return f"Return the final component of `{s[0]}`.", f"Return the file name portion of the path `{s[0]}`."
    if b == 'dirname' and top == 'path' and s[0]:
        return f"Return the directory portion of `{s[0]}`.", f"Return all but the last component of the path `{s[0]}`."
    if b == 'extname' and top == 'path' and s[0]:
        return f"Return the file extension of `{s[0]}`.", f"Return the extension of the file at `{s[0]}`."

    # process / cmd
    if b == 'spawn' and top == 'process' and s[0]:
        return f"Spawn the command `{s[0]}`.", f"Start a new process running the command `{s[0]}` and return its handle or PID."
    if b == 'kill' and top == 'process' and s[0]:
        return f"Terminate the process `{s[0]}`.", f"Send a termination signal to the process `{s[0]}`."
    if b == 'wait' and top == 'process' and s[0]:
        return f"Wait for the process `{s[0]}` to exit.", f"Block until the process `{s[0]}` finishes and return its status."
    if b == 'status' and top == 'process' and s[0]:
        return f"Return the exit status of `{s[0]}`.", f"Return the current or last exit status of the process `{s[0]}`."
    if b == 'run' and top == 'cmd' and s[0]:
        return f"Execute the shell command `{s[0]}`.", f"Run the command `{s[0]}` and return its output."

    # time / tmp / sig / socket
    if b == 'now' and top == 'time':
        return f"Return the current time.", f"Return the current time as an ISO 8601 string."
    if b == 'epoch' and top == 'time':
        return f"Return the current Unix timestamp.", f"Return the number of seconds since the Unix epoch."
    if b == 'file' and top == 'tmp':
        return f"Create and return a temporary file path.", f"Create a new temporary file and return its path."
    if b == 'dir' and top == 'tmp':
        return f"Create and return a temporary directory path.", f"Create a new temporary directory and return its path."
    if b == 'send' and top == 'sig':
        return f"Send a signal to a process.", f"Send the signal `{s[0] or 'name'}` to the process `{s[1] or 'pid'}`."
    if b == 'list' and top == 'sig':
        return f"Return the supported signal names.", f"Return an array of available signal names."
    if b == 'connect' and top == 'socket' and s[0]:
        return f"Open a connection to `{s[0]}`.", f"Connect to the address `{s[0]}` and return a socket handle."
    if b == 'send' and top == 'socket' and s[0]:
        return f"Send data over the socket `{s[0]}`.", f"Send `{s[1] or 'data'}` through the socket `{s[0]}`."
    if b == 'recv' and top == 'socket' and s[0]:
        return f"Receive data from the socket `{s[0]}`.", f"Read incoming data from the socket `{s[0]}`."
    if b == 'close' and top == 'socket' and s[0]:
        return f"Close the socket `{s[0]}`.", f"Close the connection `{s[0]}`."

    # datetime (common names)
    if b == 'from_unix' and s[0]:
        return f"Convert the Unix timestamp `{s[0]}` to a datetime string.", f"Return an ISO 8601 datetime string for the Unix timestamp `{s[0]}`."
    if b == 'to_unix' and s[0]:
        return f"Convert the datetime string `{s[0]}` to a Unix timestamp.", f"Return the number of seconds since the Unix epoch for the datetime `{s[0]}`."
    if b == 'format' and s[0]:
        return f"Format `{s[0]}` as a string.", f"Format the datetime `{s[0]}` using the pattern `{s[1] or 'pattern'}`."
    if b == 'parse' and s[0]:
        return f"Parse the datetime string `{s[0]}`.", f"Parse the string `{s[0]}` into a datetime value."
    if 'add_' in b and s[0]:
        return f"Add `{s[1] or 'n'}` {b[4:]} to `{s[0]}`.", f"Return a new datetime that is `{s[1] or 'n'}` {b[4:]} after `{s[0]}`."
    if 'sub_' in b and s[0]:
        return f"Subtract `{s[1] or 'n'}` {b[4:]} from `{s[0]}`.", f"Return a new datetime that is `{s[1] or 'n'}` {b[4:]} before `{s[0]}`."
    if 'diff_' in b and s[0] and s[1]:
        return f"Return the difference between `{s[0]}` and `{s[1]}` in {b[5:]}.", f"Compute the number of {b[5:]} between the two datetimes."

    # generic conversion/format ops
    if b == 'encode' and s[0]:
        return f"Encode `{s[0]}`.", f"Encode `{s[0]}` using `{fm}` and return the encoded result."
    if b == 'decode' and s[0]:
        return f"Decode `{s[0]}`.", f"Decode the `{fm}`-encoded value `{s[0]}`."
    if b == 'parse' and s[0]:
        return f"Parse `{s[0]}`.", f"Parse the string `{s[0]}` and return the resulting value."
    if b == 'serialize' and s[0]:
        return f"Serialize `{s[0]}` to a string.", f"Convert `{s[0]}` into its string representation."

    # generic math/stats
    if b in ('min', 'max', 'sum', 'avg', 'product', 'median', 'stddev') and s[0]:
        return f"Return the {b} of the values in `{s[0]}`.", f"Compute the {b} of the numeric values in `{s[0]}`."
    if b in ('abs', 'floor', 'ceil', 'round', 'sqrt', 'sin', 'cos', 'tan', 'log', 'exp') and s[0]:
        return f"Return the {b} of `{s[0]}`.", f"Compute the mathematical {b} of `{s[0]}`."

    # kv
    if b == 'get' and top == 'kv' and s[0]:
        return f"Return the value for `{s[1] or 'key'}` in the `{fm}` store.", f"Retrieve the value associated with `{s[1] or 'key'}`."
    if b == 'set' and top == 'kv' and s[0]:
        return f"Store `{s[2] or 'value'}` under `{s[1] or 'key'}`.", f"Associate `{s[2] or 'value'}` with the key `{s[1] or 'key'}` in the `{fm}` store."
    if b == 'has' and top == 'kv' and s[0]:
        return f"Return `true` if the `{fm}` store contains `{s[1] or 'key'}`.", f"Check whether the key `{s[1] or 'key'}` exists."
    if b == 'delete' and top == 'kv' and s[0]:
        return f"Remove the key `{s[1] or 'key'}` from the `{fm}` store.", f"Delete the key `{s[1] or 'key'}` and return the updated store."
    if b == 'keys' and top == 'kv' and s[0]:
        return f"Return the keys in the `{fm}` store.", f"Return an array of all keys."

    # http
    if b in ('get', 'post', 'put', 'delete', 'patch', 'head', 'options') and top == 'http' and s[0]:
        return f"Perform an HTTP {b.upper()} request to `{s[0]}`.", f"Send an HTTP {b.upper()} request to `{s[0]}` and return the response."
    if b == 'request' and top == 'http' and s[0]:
        return f"Perform an HTTP request described by `{s[0]}`.", f"Send the HTTP request configured in `{s[0]}` and return the response."
    if b == 'client' and top == 'http':
        return f"Create an HTTP client.", f"Create and return a new HTTP client instance."

    # assert / test
    if b == 'true' and top == 'assert' and s[0]:
        return f"Assert that `{s[0]}` is truthy.", f"Throw an assertion error if `{s[0]}` is not truthy."
    if b == 'false' and top == 'assert' and s[0]:
        return f"Assert that `{s[0]}` is falsy.", f"Throw an assertion error if `{s[0]}` is not falsy."
    if b == 'contains' and top == 'assert' and s[0]:
        return f"Assert that `{s[0]}` contains `{s[1] or 'value'}`.", f"Throw an assertion error if `{s[1] or 'value'}` is not found in `{s[0]}`."
    if b == 'throws' and top == 'assert' and s[0]:
        return f"Assert that `{s[0]}` throws an error.", f"Verify that calling `{s[0]}` raises an error."
    if b == 'run' and top == 'test':
        return f"Run the test suite.", f"Execute the configured tests and report results."
    if b == 'throughput' and top == 'benchmark':
        return f"Measure throughput.", f"Run a throughput benchmark."
    if b == 'run' and top == 'fuzz':
        return f"Run the fuzzer.", f"Execute the fuzz target."
    if b == 'for_all' and top == 'property':
        return f"Check a property for all generated inputs.", f"Run a property-based test."
    if b in ('capture', 'compare') and top == 'snapshot':
        return f"{b.capitalize()} a snapshot.", f"{b.capitalize()} the current state against a stored snapshot."

    # fallback
    sig = entry.get('signature', f"{fm}.{b}()")
    short = f"Return the result of the `{b}` function on `{fm}`."
    long = f"Calls `{sig}` with the given arguments and returns a `{ret}` result."
    return short, long


def main():
    base = Path(__file__).resolve().parents[3]
    yaml_path = base / 'docs' / 'function-docs.yaml'
    with open(yaml_path) as f:
        docs = yaml.safe_load(f) or {}

    global signature_vars
    for top, fns in docs.items():
        for fn, entry in fns.items():
            if not is_stub(entry):
                continue
            ret = entry.get('return_value') or 'type.any'
            signature_vars = [p_name(entry, i) for i in range(4)]
            desc, details = desc_for(top, fn, ret, entry)
            entry['description'] = desc
            entry['details'] = details
            # remove unhelpful auto-generated example if it is just the call stub
            exs = entry.get('examples', [])
            if exs and exs == [f"{full_module(top, fn)}.{base_fn(fn)}()"]:
                entry['examples'] = []

    with open(yaml_path, 'w') as f:
        yaml.safe_dump(docs, f, sort_keys=False, allow_unicode=True, width=120)
    print(f'Wrote {yaml_path}')


if __name__ == '__main__':
    main()
