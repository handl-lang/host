#!/usr/bin/env bash
# dev-link.sh — regenerate the `[patch."https://github.com/handl-lang/*"]`
# tables in a repo's root Cargo.toml from the sibling checkouts
# (ADR-0128 §Resolution: git deps + `[patch]`, local-only).
#
#   scripts/dev-link.sh            # patch this repo (default: cwd)
#   scripts/dev-link.sh ../host-cli     # patch a specific sibling checkout
#
# The generated tables replace the region between
#
#   # dev-link: begin
#   # dev-link: end
#
# in <repo>/Cargo.toml (markers are created on first run, appended at EOF).
# For every sibling repo R referenced by a `git = "…/handl-lang/R"` dep in
# the target's manifests — transitively closed, because a patched-in crate can
# git-dep a repo the target never names — each crate of R is patched to its
# sibling path: the workspace members under `R/{crates,modules}/`, plus R's own
# root `[package]` when it is a single-crate repo (the ADR-0128 `lib-*` ports).
#
# Manifests that also carry hand-maintained tables outside the region (e.g.
# `host`) are *not* rewritten silently: a second table for the same
# `[patch."…/R"]` URL makes cargo reject the manifest ("duplicate key"), so the
# run aborts with a report instead. `DEV_LINK_FORCE=1` folds every table into
# the region (hand-written paths win), `DEV_LINK_NO_PRUNE=1` skips the
# unused-patch prune described below. CI strips/ignores the tables and resolves
# the pinned revs.
#
# After writing, entries cargo reports as unused are dropped (they are
# resolution-neutral but make the lockfile byte-unstable — see
# `scripts/dev-link-prune.py`), then the lock is re-resolved.
set -euo pipefail

REPO="${1:-.}"
ORG="$(cd "$REPO/.." && pwd)"
TARGET="$ORG/$(basename "$(cd "$REPO" && pwd)")"

python3 - "$TARGET" "$ORG" <<'PY'
import os, re, sys, pathlib

target = pathlib.Path(sys.argv[1])
org = pathlib.Path(sys.argv[2])
root_toml = target / "Cargo.toml"
base = "https://github.com/handl-lang/"
BEGIN = "# dev-link: begin"
END = "# dev-link: end"

# Repos this repo references via git deps (any Cargo.toml, any depth) —
# plus one hop through the referenced repos' own manifests: patched-in
# sibling crates git-dep further repos (e.g. host's own
# `[patch."…/host"]` table exists because kit-canonical's manifest
# git-deps host crates).
pat = re.compile(r'git = "' + re.escape(base) + r'([A-Za-z0-9._-]+)"')

def refs_of(d: pathlib.Path):
    out = set()
    for toml in d.rglob("Cargo.toml"):
        if "target" in toml.parts:
            continue
        out.update(pat.findall(toml.read_text()))
    return out

refd = refs_of(target)
seen = set()
while True:
    fresh = set()
    for repo in refd - seen:
        d = org / repo
        if d.is_dir():
            fresh |= refs_of(d)
    seen |= refd
    if not (fresh - refd):
        break
    refd |= fresh

# Map repo -> [(package name, sibling relative path)] for referenced repos.
#
# Two layouts are supported: a workspace of `crates/*` + `modules/*`
# members, and the single-crate-at-root layout (`lib-*` ports after the
# ADR-0128 split), which has no `crates/`/`modules/` directory at all.
def packages(repo_dir: pathlib.Path):
    out = []
    root_toml = repo_dir / "Cargo.toml"
    if root_toml.is_file():
        text = root_toml.read_text()
        m = re.search(r'^name = "([^"]+)"', text, re.M)
        if m and re.search(r"^\[package\]", text, re.M):
            out.append((m.group(1), repo_dir.relative_to(org).as_posix()))
    for group in ("crates", "modules"):
        g = repo_dir / group
        if not g.is_dir():
            continue
        for toml in sorted(g.glob("*/Cargo.toml")):
            m = re.search(r'^name = "([^"]+)"', toml.read_text(), re.M)
            if m:
                out.append((m.group(1), toml.parent.relative_to(org).as_posix()))
    return out

blocks = []
missing = []
for repo in sorted(refd):
    d = org / repo
    if not d.is_dir():
        missing.append(repo)
        continue
    lines = [f'[patch."{base}{repo}"]']
    for name, rel in packages(d):
        lines.append(f"{name} = {{ path = \"../{rel}\" }}")
    blocks.append("\n".join(lines))

for repo in missing:
    print(f"dev-link: warning: referenced repo ../{repo} not checked out", file=sys.stderr)

text = root_toml.read_text()
region = "\n".join([BEGIN] + blocks + [END])

# Entries already present anywhere in the manifest. Some repos (e.g. `host`)
# keep hand-maintained tables *outside* the region; emitting a second table for
# the same `[patch."…/R"]` URL makes cargo reject the manifest with
# "duplicate key", so check before writing.
existing = {}  # (url, name) -> path
cur = None
for line in text.splitlines():
    m = re.match(r'^\[patch\."([^"]+)"\]', line)
    if m:
        cur = m.group(1)
        continue
    if line.startswith("["):
        cur = None
        continue
    mm = re.match(r'^([A-Za-z0-9_-]+)\s*=\s*\{\s*path\s*=\s*"([^"]+)"\s*\}', line)
    if cur and mm:
        existing[(cur, mm.group(1))] = mm.group(2)

managed = set()
if BEGIN in text and END in text:
    for line in text.split(BEGIN, 1)[1].split(END, 1)[0].splitlines():
        m = re.match(r'^\[patch\."([^"]+)"\]', line)
        if m:
            cur = m.group(1)
            continue
        if line.startswith("["):
            cur = None
            continue
        mm = re.match(r'^([A-Za-z0-9_-]+)\s*=', line)
        if cur and mm:
            managed.add((cur, mm.group(1)))

force = os.environ.get("DEV_LINK_FORCE") == "1"
generated_urls = {base + r for r in refd if (org / r).is_dir()}
collide = sorted(
    url_name for url_name in set(existing) - managed if url_name[0] in generated_urls
)
if collide and not force:
    plural = "entry" if len(collide) == 1 else "entries"
    print(
        f"dev-link: refusing to write — {len(collide)} patch {plural} outside the"
        " managed region collide with tables this run would emit:",
        file=sys.stderr,
    )
    for url, name in collide[:8]:
        print(f"  [{url}] {name}", file=sys.stderr)
    if len(collide) > 8:
        print(f"  … and {len(collide) - 8} more", file=sys.stderr)
    print(
        "Consolidate them by hand, or re-run with DEV_LINK_FORCE=1 to fold every"
        " table into the managed region.",
        file=sys.stderr,
    )
    sys.exit(2)

if force:
    # Fold everything into one region: generated members first, then the
    # hand-maintained entries (their paths win — they encode cases like
    # `handl-plugin-ffi -> ../kit-ffi/modules/ffi` that the scan cannot derive).
    merged = {}
    for repo in sorted(refd):
        d = org / repo
        if d.is_dir():
            for name, rel in packages(d):
                merged[(base + repo, name)] = f"../{rel}"
    merged.update(existing)
    by_url = {}
    for (url, name), path in merged.items():
        by_url.setdefault(url, []).append((name, path))
    blocks = [
        "\n".join([f'[patch."{url}"]'] + [f'{n} = {{ path = "{p}" }}' for n, p in members])
        for url, members in sorted(by_url.items())
    ]
    region = "\n".join([BEGIN] + blocks + [END])
    kept, in_patch = [], False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped in (BEGIN, END):
            continue  # markers go with the tables they framed
        if line.startswith("[patch."):
            in_patch = True
            continue
        if in_patch:
            if line.startswith("["):
                in_patch = False
            else:
                continue
        kept.append(line)
    text = "\n".join(kept).rstrip() + "\n\n" + region + "\n"
elif BEGIN in text and END in text:
    text = re.sub(
        re.escape(BEGIN) + r".*?" + re.escape(END),
        lambda _: region,
        text,
        count=1,
        flags=re.S,
    )
else:
    text = text.rstrip() + "\n\n" + region + "\n"
root_toml.write_text(text)
print(f"dev-link: wrote {len(blocks)} patch table(s) to {root_toml}")
PY

# A patch entry for a crate nothing depends on does not participate in
# resolution, but cargo still records it as `[[patch.unused]]` in the lockfile —
# and serializes that list in HashMap order, which makes the lockfile
# byte-unstable and `cargo … --locked` fail forever ("cannot update the lock
# file … because --locked was passed") as soon as two patch sources contribute
# an unused entry. Resolve once, delete the manifest entries cargo reports as
# unused, resolve again. Set DEV_LINK_NO_PRUNE=1 to keep the full table set.
if [ "${DEV_LINK_NO_PRUNE:-0}" != "1" ] && [ -f "$TARGET/Cargo.lock" ] \
   && command -v cargo >/dev/null 2>&1; then
  prune_py="$(dirname "${BASH_SOURCE[0]:-$0}")/dev-link-prune.py"
  [ -f "$prune_py" ] || prune_py="$ORG/host/scripts/dev-link-prune.py"
  if [ -f "$prune_py" ] \
     && (cd "$TARGET" && cargo metadata --format-version 1 >/dev/null 2>&1); then
    report="$(python3 "$prune_py" "$TARGET" 2>/dev/null || true)"
    case "$report" in
      ""|removed=0*) ;;
      *)
        (cd "$TARGET" && cargo metadata --format-version 1 >/dev/null 2>&1) || true
        echo "dev-link: pruned unused patches ($report)"
        ;;
    esac
  else
    echo "dev-link: warning: could not resolve — skipped unused-patch prune" >&2
  fi
fi
