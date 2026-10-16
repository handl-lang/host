#!/usr/bin/env bash
# ADR-0129 Phase 2 — split project/ docs into one orphan `project` branch per repo.
#
#   scripts/split-project-docs.sh check         invariant: every file assigned once
#   scripts/split-project-docs.sh paths <dir>   write <dir>/paths-<repo>.txt
#   scripts/split-project-docs.sh emit <repo>   bundle <STAGE>/<repo>.bundle
#   scripts/split-project-docs.sh emit-all      bundle every non-empty repo
#   scripts/split-project-docs.sh seed <repo> <target-repo-dir>
#                                               fetch the bundle as branch
#                                               `project` + mount the worktree
#
# STAGE defaults to <parent-of-platform>/.project-docs-bundles (outside every
# repo). Requires git-filter-repo on PATH.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PY=${PY:-python3}
STAGE=${STAGE:-$(dirname "$ROOT")/.project-docs-bundles}
FR=$(command -v git-filter-repo || true)

require_fr() {
  if [ -n "$FR" ]; then return; fi
  cat >&2 <<'EOF'
git-filter-repo not found on PATH. Install it with one of:
  pipx install git-filter-repo
  curl -fsSL -o ~/.local/bin/git-filter-repo \
    https://raw.githubusercontent.com/newren/git-filter-repo/main/git-filter-repo
  chmod +x ~/.local/bin/git-filter-repo
EOF
  exit 1
}

emit() {
  local repo=$1
  require_fr
  mkdir -p "$STAGE"
  local work="$STAGE/work-$repo"
  rm -rf "$work"
  echo ">> cloning platform for '$repo'"
  git clone --quiet --no-local "$ROOT" "$work"
  "$PY" "$ROOT/scripts/docs_split.py" paths "$repo" > "$STAGE/paths-$repo.txt"
  local n; n=$(wc -l < "$STAGE/paths-$repo.txt")
  echo ">> filtering $repo ($n paths)"
  (
    cd "$work"
    "$FR" --force --quiet --refs main \
      --paths-from-file "$STAGE/paths-$repo.txt" \
      --path-rename 'project/:' >/dev/null
    git bundle create "$STAGE/$repo.bundle" main >/dev/null
  )
  echo "   -> $STAGE/$repo.bundle"
}

seed() {
  local repo=$1 target=$2
  [ -f "$STAGE/$repo.bundle" ] || emit "$repo"
  (
    cd "$target"
    git fetch --quiet "$STAGE/$repo.bundle" main:project
    git worktree add project project
    if [ -f .gitignore ] && grep -qxF '/project/' .gitignore; then
      :
    else
      printf '/project/\n' >> .gitignore
    fi
    git add .gitignore
    git commit --quiet -m "chore: ignore the project worktree (ADR-0129)"
  )
  echo "   -> $target/project mounted on branch 'project'"
}

case "${1:-}" in
  check)   "$PY" "$ROOT/scripts/docs_split.py" check ;;
  paths)   "$PY" "$ROOT/scripts/docs_split.py" paths-all "${2:?dir}" ;;
  emit)    emit "${2:?repo}" ;;
  emit-all)
    for r in $("$PY" - "$ROOT/project/docs-split.yaml" <<'PY'
import sys, yaml
d = yaml.safe_load(open(sys.argv[1]))
for repo, sec in (d.get("repos") or {}).items():
    if any(v for v in (sec or {}).values()):
        print(repo)
PY
    ); do emit "$r"; done ;;
  seed)    seed "${2:?repo}" "${3:?target-dir}" ;;
  *)       sed -n '2,18p' "$0"; exit 2 ;;
esac
