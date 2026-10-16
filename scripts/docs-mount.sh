#!/usr/bin/env bash
# ADR-0129 Phase 3.3 — mount / inspect the `project` worktree of every sibling
# repo in the org.
#
#   scripts/docs-mount.sh            mount what it can, report the rest
#   scripts/docs-mount.sh --status   report only (no changes)
#
# For each repo in project/docs-split.yaml:
#   * repo missing            -> "missing repo"      (created when ADR-0128 extracts it)
#   * branch `project` exists -> ensure the worktree is mounted
#   * staged bundle exists    -> `split-project-docs.sh seed`
#   * otherwise               -> "pending"
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ORG=$(dirname "$ROOT")
PY=${PY:-python3}
STAGE=${STAGE:-$ORG/.project-docs-bundles}
STATUS_ONLY=0
[ "${1:-}" = "--status" ] && STATUS_ONLY=1

printf '%-16s %-10s %s\n' REPO STATE DETAIL
printf '%-16s %-10s %s\n' ---- ----- ------

for repo in $("$PY" "$ROOT/scripts/docs_split.py" repos); do
  dir="$ORG/$repo"
  if [ ! -d "$dir/.git" ]; then
    printf '%-16s %-10s %s\n' "$repo" "missing" "no checkout at $dir"
    continue
  fi
  if git -C "$dir" show-ref --quiet --verify refs/heads/project; then
    if git -C "$dir" worktree list --porcelain | grep -qx "worktree $dir/project"; then
      printf '%-16s %-10s %s\n' "$repo" "mounted" "$dir/project"
    elif [ "$STATUS_ONLY" = 1 ]; then
      printf '%-16s %-10s %s\n' "$repo" "unmounted" "branch exists, run without --status"
    else
      git -C "$dir" worktree add -q project project
      printf '%-16s %-10s %s\n' "$repo" "mounted" "$dir/project"
    fi
  elif [ -f "$STAGE/$repo.bundle" ]; then
    if [ "$STATUS_ONLY" = 1 ]; then
      printf '%-16s %-10s %s\n' "$repo" "seedable" "bundle staged"
    else
      "$ROOT/scripts/split-project-docs.sh" seed "$repo" "$dir" >/dev/null
      printf '%-16s %-10s %s\n' "$repo" "seeded" "$dir/project"
    fi
  else
    printf '%-16s %-10s %s\n' "$repo" "pending" "no branch, no bundle"
  fi
done
