#!/usr/bin/env bash
# Convenience wrapper for the Python harness.
set -euo pipefail
cd "$(dirname "$0")"
python3 run.py "$@"
