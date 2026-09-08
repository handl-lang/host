#!/usr/bin/env bash
# Dogfood CI gate: hnl fmt --check && hnl lint && hnl typecheck && hnl test
# over handl-packages/ and examples/.
# The gate is currently informational in CI because it is not yet green.
set -euo pipefail

HNL=./target/debug/hnl
status=0

echo "== hnl dogfood gate: handl-packages/ examples/ =="

# fmt --check over every .hnl file.
while IFS= read -r -d '' f; do
  if ! "$HNL" fmt --check "$f"; then
    status=1
  fi
done < <(find handl-packages examples -name '*.hnl' -print0 | sort -z)

# lint over every .hnl file.
while IFS= read -r -d '' f; do
  if ! "$HNL" lint "$f"; then
    status=1
  fi
done < <(find handl-packages examples -name '*.hnl' -print0 | sort -z)

# typecheck over every .hnl file.
while IFS= read -r -d '' f; do
  if ! "$HNL" typecheck "$f"; then
    status=1
  fi
done < <(find handl-packages examples -name '*.hnl' -print0 | sort -z)

# test discovers test-kind declarations across the two trees.
if ! "$HNL" test handl-packages examples; then
  status=1
fi

exit "$status"
