#!/usr/bin/env python3
"""stage_stub.py — fixture backend for handl-kit-mlopt tests.

Speaks the documented ``python -m mlopt stage <name>`` protocol:

- argv: last argument is the stage name
  (``lift``/``search``/``verify``/``deliver``)
- stdin: exactly one JSON value — the stage input (a model-ref string
  for ``lift``, the pipeline record object thereafter)
- stdout: exactly one JSON value — the stage output record
- ``explode`` exits 3 with stderr diagnostics; ``badjson`` exits 0
  with non-JSON stdout — the two honest-failure shapes the Rust side
  must surface as ``EvalError``.

Unlike a canned echo this stub *threads the record*: ``search`` and
``verify`` read the incoming object and extend it, so tests exercise
the real stdin -> stdout JSON channel end to end.
"""

import json
import sys


def main() -> int:
    stage = sys.argv[-1] if len(sys.argv) > 1 else ""
    raw = sys.stdin.read()
    try:
        inp = json.loads(raw) if raw.strip() else None
    except json.JSONDecodeError as e:
        print(f"stage_stub: bad stdin JSON: {e}", file=sys.stderr)
        return 4

    if stage == "lift":
        model = inp if isinstance(inp, str) else "demo.torch"
        print(json.dumps({
            "stage": "lift",
            "model": model,
            "ir": {"nodes": 3},
            "ok": True,
        }))
    elif stage == "search":
        rec = dict(inp) if isinstance(inp, dict) else {}
        rec.update({"stage": "search", "laws_applied": 7, "ok": True})
        print(json.dumps(rec))
    elif stage == "verify":
        rec = dict(inp) if isinstance(inp, dict) else {}
        if rec.get("laws_applied") != 7:
            print("stage_stub: verify: search did not run", file=sys.stderr)
            return 5
        print(json.dumps({
            "stage": "verify",
            "ok": True,
            "model": rec.get("model"),
            "verified": {"passed": True},
        }))
    elif stage == "deliver":
        rec = dict(inp) if isinstance(inp, dict) else {}
        out = rec.get("out")
        if not isinstance(out, str) or not out:
            print("stage_stub: deliver: record carries no 'out'", file=sys.stderr)
            return 6
        if rec.get("verified", {}).get("passed") is not True:
            print("stage_stub: deliver: verify report did not pass", file=sys.stderr)
            return 7
        rec.update({
            "stage": "deliver",
            "ok": True,
            "delivered": True,
            "artifact": {"path": out, "format": "hnl"},
        })
        print(json.dumps(rec))
    elif stage == "vok":
        # A record carrying the current schema pin — the kit's
        # tolerant reader must accept it.
        print(json.dumps({"stage": "vok", "ok": True, "v": 3}))
    elif stage == "vfuture":
        # A record stamped newer than the kit's schema — must be a
        # loud EvalError, never a silent pass.
        print(json.dumps({"stage": "vfuture", "ok": True, "v": 99}))
    elif stage == "vbad":
        # A malformed (non-int) version field — same loud decline.
        print(json.dumps({"stage": "vbad", "ok": True, "v": "two"}))
    elif stage == "explode":
        print("stage_stub: deliberate failure for explode", file=sys.stderr)
        return 3
    elif stage == "badjson":
        print("this is not json")
        return 0
    else:
        print(f"stage_stub: unknown stage {stage}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
