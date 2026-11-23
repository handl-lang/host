//! mlopt change-set manifest → `hnl-engine-codegen` apply bridge
//! (PL-004 T-093).
//!
//! mlopt's emit side (`mlopt.emit.emit_changes` →
//! `mlopt.manifest.write_manifest`, consumed by `mlopt export
//! --manifest DIR` and `python -m mlopt stage deliver`) produces a
//! *desired-state request*: `manifest.json` + a content-addressed
//! blob store (`<dir>/blobs/<blake3-hex>`), never a filesystem
//! mutation. This module is the consumer half — it parses that wire
//! format and reconciles it through the kernel's
//! [`Engine::plan`]/[`Engine::apply`] lifecycle over [`FsStore`], so
//! mlopt deliverables get the same journaled, atomic, hash-checked
//! writes every other codegen emitter gets.
//!
//! ## Wire format (mirrors `mlopt/manifest.py`)
//!
//! `manifest.json` carries the kernel `Manifest` provenance fields
//! (`target`/`generator_version`/`source_hashes`/`artifacts`/`run_id`)
//! plus emit-side extras: `"v"` (manifest schema pin),
//! `"hash_algorithm"` (must be `"blake3-256"` — the kernel's own
//! `ContentHash` algorithm; `blake2b-256` manifests from the v4 record
//! era are refused), `"manifest_dir"`, and `"changes"` — the desired
//! set as `{"op", "artifact", "before_hash", "blob_ref"|"content_b64",
//! "hash"}` wire dicts.
//!
//! ## Apply-root convention
//!
//! Change `artifact` refs are verbatim emitter paths (e.g. the
//! pipeline record's `out`, typically relative to the Python
//! process's cwd). [`apply_manifest`]/[`plan_manifest`] take an
//! explicit `root: Option<&Path>` for the [`FsStore`]; `None` means
//! the current working directory — apply from the same cwd the
//! emitter ran in and refs land where `out` pointed. Absolute or
//! `..`-escaping refs are rejected by the kernel's
//! `ArtifactRef::validate_fs_relative`, fail-closed.
//!
//! ## Where `Emitter::verify` would slot
//!
//! mlopt's verify gate (`lower(verify=True)` — the `hnl-run` eval
//! gate in `optimize.py`/`stage deliver`) is **producer-side**: it
//! runs on the term/module *before* `emit_changes` renders the
//! manifest, so by the time this shim sees a manifest the artifact
//! is already verified-or-declined. The kernel's
//! [`hnl_engine_codegen::Emitter::verify`] hook — `verify(&self, ir,
//! changes) -> Result<(), CodegenError>` seeing both IR and desired
//! state — is the slot for a *consumer-side* gate if a Rust emitter
//! ever materializes mlopt terms directly (e.g. a `kit-mlopt` native
//! emitter over `lib-kernel-ir`/`hnl` terms): it would run between
//! `materialize` and `Engine::apply`, mirroring how mlopt's gate
//! runs between `lower` and manifest emission. No such emitter
//! exists yet — this bridge only applies already-verified desired
//! state, and [`apply_manifest`] deliberately performs no semantic
//! re-verification (hash/drift checks only, which is the kernel's
//! own guarantee layer).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use hnl_engine_codegen::change::Change;
use hnl_engine_codegen::engine::{ApplyOptions, ApplyReport, Engine, RunMeta};
use hnl_engine_codegen::error::CodegenError;
use hnl_engine_codegen::manifest::Manifest;
use hnl_engine_codegen::plan::Plan;
use hnl_engine_codegen::store::{ArtifactStore, FsStore};
use hnl_engine_codegen::types::{ArtifactRef, ContentHash};
use serde_json::Value;

/// `manifest.json` file name inside a manifest dir (mlopt
/// `manifest.MANIFEST_FILENAME`).
pub const MANIFEST_FILENAME: &str = "manifest.json";

/// The only `hash_algorithm` this shim accepts — BLAKE3-256, the
/// kernel's `ContentHash` algorithm, so declared hashes compare
/// directly against `ContentHash::of` with no translation.
pub const HASH_ALGORITHM: &str = "blake3-256";

/// Blob-store directory name inside a manifest dir (mlopt
/// `manifest.BLOB_DIRNAME`).
pub const BLOB_DIRNAME: &str = "blobs";

/// A parsed mlopt manifest — provenance fields plus the desired
/// change set, payloads still blob-store-relative until
/// [`resolve_changes`] loads them.
#[derive(Clone, Debug)]
pub struct ManifestDoc {
    /// Store dir the manifest was read from (absolute).
    pub dir: PathBuf,
    /// Generator/target name — becomes the kernel manifest owner.
    pub target: String,
    /// Generator version string (e.g. `"mlopt-stage-deliver/v5"`).
    pub generator_version: String,
    /// Source hashes as recorded — hex strings kept verbatim for
    /// `RunMeta` (non-hex values are dropped there; they are
    /// provenance, not verified inputs).
    pub source_hashes: BTreeMap<String, String>,
    /// Journal run id recorded by the emitter (may be empty).
    pub run_id: String,
    /// Desired-state wire changes, in manifest order.
    pub changes: Vec<WireChange>,
}

/// One change-set entry parsed from `manifest.json` — the wire shape
/// before blob resolution.
#[derive(Clone, Debug)]
pub enum WireChange {
    /// `{"op":"put", ...}` — payload referenced by `blob_ref`
    /// (`blobs/<hash>`) or inlined as `content_b64`.
    Put {
        artifact: String,
        before_hash: Option<String>,
        blob_ref: Option<String>,
        content_b64: Option<String>,
        hash: Option<String>,
    },
    /// `{"op":"delete", "artifact": ...}` — emitter-declared cleanup.
    Delete { artifact: String },
}

fn manifest_err(msg: impl Into<String>) -> CodegenError {
    CodegenError::Manifest(msg.into())
}

fn req_str<'a>(v: &'a Value, key: &str, ctx: &str) -> Result<&'a str, CodegenError> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| manifest_err(format!("{ctx}: missing/invalid `{key}`")))
}

/// Parse `manifest.json` inside `manifest_dir` into a [`ManifestDoc`].
///
/// Fails closed on: missing/malformed JSON, a `hash_algorithm` other
/// than [`HASH_ALGORITHM`] (the `blake2b-256` v4-era manifests are
/// refused — regenerate via a fresh `deliver`), and malformed change
/// entries. Payload bytes are NOT loaded here — see
/// [`resolve_changes`].
pub fn read_manifest(manifest_dir: &Path) -> Result<ManifestDoc, CodegenError> {
    let dir = manifest_dir.to_path_buf();
    let path = dir.join(MANIFEST_FILENAME);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| manifest_err(format!("cannot read {}: {e}", path.display())))?;
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| manifest_err(format!("{}: invalid JSON: {e}", path.display())))?;

    if let Some(alg) = doc.get("hash_algorithm").and_then(Value::as_str)
        && alg != HASH_ALGORITHM
    {
        return Err(manifest_err(format!(
            "{}: hash_algorithm {alg:?} — this shim speaks {HASH_ALGORITHM:?} \
             (blake2b-256 manifests predate the BLAKE3 migration; regenerate)",
            path.display()
        )));
    }

    let mut source_hashes = BTreeMap::new();
    if let Some(sh) = doc.get("source_hashes").and_then(Value::as_object) {
        for (k, v) in sh {
            if let Some(s) = v.as_str() {
                source_hashes.insert(k.clone(), s.to_string());
            }
        }
    }

    let mut changes = Vec::new();
    for (i, w) in doc
        .get("changes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let ctx = format!("changes[{i}]");
        let op = req_str(w, "op", &ctx)?;
        let artifact = req_str(w, "artifact", &ctx)?.to_string();
        match op {
            "put" => changes.push(WireChange::Put {
                artifact,
                before_hash: w
                    .get("before_hash")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                blob_ref: w
                    .get("blob_ref")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content_b64: w
                    .get("content_b64")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                hash: w.get("hash").and_then(Value::as_str).map(str::to_string),
            }),
            "delete" => changes.push(WireChange::Delete { artifact }),
            other => {
                return Err(manifest_err(format!(
                    "{ctx}: unknown op {other:?} (put|delete)"
                )));
            }
        }
    }

    Ok(ManifestDoc {
        dir,
        target: doc
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        generator_version: doc
            .get("generator_version")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        source_hashes,
        run_id: doc
            .get("run_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        changes,
    })
}

/// Load a put payload: `content_b64` inline, else the
/// `blob_ref` (`blobs/<hash>`) file under `dir`. The declared `hash`
/// — and, for blob refs, the filename itself — must equal
/// `ContentHash::of(bytes)`; a mismatch is a corrupt manifest, not a
/// drift event.
fn load_payload(
    doc: &ManifestDoc,
    blob_ref: Option<&str>,
    content_b64: Option<&str>,
    declared: Option<&str>,
    artifact: &str,
) -> Result<Vec<u8>, CodegenError> {
    let bytes = if let Some(b64) = content_b64 {
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| manifest_err(format!("{artifact}: bad content_b64: {e}")))?
    } else if let Some(br) = blob_ref {
        // blob_ref is store-relative (`blobs/<hash>`); refuse escapes.
        let rel = Path::new(br);
        if rel.is_absolute()
            || rel
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(manifest_err(format!("{artifact}: unsafe blob_ref {br:?}")));
        }
        let p = doc.dir.join(rel);
        std::fs::read(&p).map_err(|e| {
            manifest_err(format!("{artifact}: cannot read blob {}: {e}", p.display()))
        })?
    } else {
        return Err(manifest_err(format!(
            "{artifact}: put change carries neither blob_ref nor content_b64"
        )));
    };

    let actual = ContentHash::of(&bytes);
    if let Some(h) = declared
        && ContentHash::from_hex(h).ok() != Some(actual)
    {
        return Err(manifest_err(format!(
            "{artifact}: declared hash {h} != content hash {}",
            actual.to_hex()
        )));
    }
    if let Some(br) = blob_ref {
        let name = Path::new(br).file_name().and_then(|s| s.to_str());
        if let Some(name) = name
            && name != actual.to_hex()
        {
            return Err(manifest_err(format!(
                "{artifact}: blob_ref {br:?} filename != content hash {}",
                actual.to_hex()
            )));
        }
    }
    Ok(bytes)
}

/// Resolve a parsed [`ManifestDoc`] into kernel [`Change`]s — blob
/// payloads loaded and hash-verified. This is the manifest→kernel
/// conversion the apply path needs; it performs no store access.
pub fn resolve_changes(doc: &ManifestDoc) -> Result<Vec<Change>, CodegenError> {
    let mut out = Vec::with_capacity(doc.changes.len());
    for w in &doc.changes {
        match w {
            WireChange::Delete { artifact } => out.push(Change::delete(artifact.clone())),
            WireChange::Put {
                artifact,
                before_hash,
                blob_ref,
                content_b64,
                hash,
            } => {
                let content = load_payload(
                    doc,
                    blob_ref.as_deref(),
                    content_b64.as_deref(),
                    hash.as_deref(),
                    artifact,
                )?;
                let before =
                    match before_hash {
                        Some(h) => Some(ContentHash::from_hex(h).map_err(|e| {
                            manifest_err(format!("{artifact}: bad before_hash: {e}"))
                        })?),
                        None => None,
                    };
                let aref = ArtifactRef::new(artifact.clone());
                out.push(Change::put_with_before(aref, before, content));
            }
        }
    }
    Ok(out)
}

/// Convenience: read + resolve in one call.
pub fn manifest_changes(manifest_dir: &Path) -> Result<Vec<Change>, CodegenError> {
    let doc = read_manifest(manifest_dir)?;
    resolve_changes(&doc)
}

/// The apply root: caller-supplied, else the process cwd (see module
/// docs — emitter `out` paths are cwd-relative).
fn resolve_root(root: Option<&Path>) -> Result<PathBuf, CodegenError> {
    match root {
        Some(r) => Ok(r.to_path_buf()),
        None => std::env::current_dir()
            .map_err(|e| manifest_err(format!("cannot resolve apply root: {e}"))),
    }
}

/// `RunMeta` from the manifest's provenance fields. `source_hashes`
/// values that aren't 64-hex `ContentHash`es are dropped — they are
/// emit-side provenance the kernel records verbatim but never
/// verifies.
fn run_meta(doc: &ManifestDoc) -> RunMeta {
    let mut meta = RunMeta::new(
        if doc.target.is_empty() {
            "mlopt-manifest".to_string()
        } else {
            doc.target.clone()
        },
        if doc.generator_version.is_empty() {
            "mlopt/unknown".to_string()
        } else {
            doc.generator_version.clone()
        },
    );
    meta.run_id = doc.run_id.clone();
    for (k, v) in &doc.source_hashes {
        if let Ok(h) = ContentHash::from_hex(v) {
            meta.source_hashes.insert(k.clone(), h);
        }
    }
    meta
}

/// Load the kernel `Manifest` a previous apply stamped for this
/// target (`.handl/codegen/manifest/<target>.json` under the store
/// root), so manifest-owned stale artifacts reconcile as deletes.
/// Absent → `None`; present-but-corrupt → fail closed.
fn prior_manifest<S: ArtifactStore + ?Sized>(
    store: &S,
    target: &str,
) -> Result<Option<Manifest>, CodegenError> {
    if target.is_empty() {
        return Ok(None);
    }
    let mref = Manifest::artifact_ref(target);
    let Some(bytes) = store.read(&mref)? else {
        return Ok(None);
    };
    let m: Manifest = serde_json::from_slice(&bytes)
        .map_err(|e| manifest_err(format!("prior manifest {} is corrupt: {e}", mref.as_str())))?;
    Ok(Some(m))
}

/// Plan-only pass: diff the manifest's desired state against the
/// store without mutating — the verify/dry-run half of the bridge.
/// `root` is the [`FsStore`] root (see module docs).
pub fn plan_manifest(manifest_dir: &Path, root: Option<&Path>) -> Result<Plan, CodegenError> {
    let doc = read_manifest(manifest_dir)?;
    let changes = resolve_changes(&doc)?;
    let store = FsStore::new(resolve_root(root)?);
    let engine = Engine::new(&store);
    let prior = prior_manifest(&store, &doc.target)?;
    engine.plan(&changes, prior.as_ref())
}

/// Apply the manifest's desired state through `Engine::apply`:
/// journaled write-ahead, sorted-order, atomic per-artifact
/// (`FsStore` temp+rename), post-write hash verification, and a
/// stamped kernel manifest on a clean run — full lifecycle, not a
/// bare copy loop.
///
/// `root` is the [`FsStore`] root (see module docs); `opts` is the
/// kernel [`ApplyOptions`] (`resume`/`keep_going`/`on_error`).
/// Returns the kernel [`ApplyReport`] verbatim — `report.clean()` is
/// the success predicate, `report.manifest_written` confirms the
/// ownership stamp.
pub fn apply_manifest(
    manifest_dir: &Path,
    root: Option<&Path>,
    opts: &ApplyOptions,
) -> Result<ApplyReport, CodegenError> {
    let doc = read_manifest(manifest_dir)?;
    let changes = resolve_changes(&doc)?;
    let store = FsStore::new(resolve_root(root)?);
    let engine = Engine::new(&store);
    let prior = prior_manifest(&store, &doc.target)?;
    engine.apply(&changes, prior.as_ref(), &run_meta(&doc), opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a manifest dir the way `mlopt.manifest.write_manifest`
    /// does: `manifest.json` + `blobs/<hash>` payloads.
    fn stage_manifest(dir: &Path, changes: &[(&str, &[u8])]) -> ManifestDoc {
        std::fs::create_dir_all(dir.join(BLOB_DIRNAME)).unwrap();
        let mut wires = Vec::new();
        for (artifact, bytes) in changes {
            let h = ContentHash::of(bytes);
            let hex = h.to_hex();
            std::fs::write(dir.join(BLOB_DIRNAME).join(&hex), bytes).unwrap();
            wires.push(serde_json::json!({
                "op": "put",
                "artifact": artifact,
                "before_hash": null,
                "blob_ref": format!("{BLOB_DIRNAME}/{hex}"),
                "hash": hex,
            }));
        }
        let doc = serde_json::json!({
            "v": 1,
            "hash_algorithm": HASH_ALGORITHM,
            "target": "demo",
            "generator_version": "mlopt-stage-deliver/v5",
            "source_hashes": {},
            "artifacts": [],
            "run_id": "test-run",
            "changes": wires,
        });
        std::fs::write(
            dir.join(MANIFEST_FILENAME),
            serde_json::to_string_pretty(&doc).unwrap(),
        )
        .unwrap();
        read_manifest(dir).unwrap()
    }

    #[test]
    fn apply_writes_artifacts_and_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let mdir = tmp.path().join("out.hnl.manifest");
        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        stage_manifest(
            &mdir,
            &[("out.hnl", b"hello\n"), ("w.safetensors", b"\x00\x01")],
        );

        // Plan first: two creates, zero mutations yet.
        let plan = plan_manifest(&mdir, Some(&root)).unwrap();
        assert_eq!(plan.creates().count(), 2);
        assert!(!root.join("out.hnl").exists());

        let report = apply_manifest(&mdir, Some(&root), &ApplyOptions::default()).unwrap();
        assert!(report.clean(), "failures: {:?}", report.failures);
        assert!(report.manifest_written);
        assert_eq!(std::fs::read(root.join("out.hnl")).unwrap(), b"hello\n");
        assert_eq!(
            std::fs::read(root.join("w.safetensors")).unwrap(),
            b"\x00\x01"
        );

        // Second apply is a no-op convergence (unchanged, not rewrite).
        let report2 = apply_manifest(&mdir, Some(&root), &ApplyOptions::default()).unwrap();
        assert!(report2.clean());
        assert_eq!(report2.unchanged, 2);
    }

    #[test]
    fn apply_is_idempotent_and_journaled() {
        let tmp = tempfile::tempdir().unwrap();
        let mdir = tmp.path().join("m");
        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        stage_manifest(&mdir, &[("a.hnl", b"a")]);
        apply_manifest(&mdir, Some(&root), &ApplyOptions::default()).unwrap();
        // Journal + kernel manifest landed under the reserved prefix.
        assert!(root.join(".handl/codegen").exists());
    }

    #[test]
    fn corrupt_blob_hash_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mdir = tmp.path().join("m");
        stage_manifest(&mdir, &[("a.hnl", b"a")]);
        // Corrupt the blob in place — declared hash no longer matches.
        let blob = mdir.join(BLOB_DIRNAME).join(ContentHash::of(b"a").to_hex());
        std::fs::write(&blob, b"tampered").unwrap();
        let err = manifest_changes(&mdir).unwrap_err();
        assert!(format!("{err}").contains("declared hash"), "{err}");
    }

    #[test]
    fn foreign_hash_algorithm_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mdir = tmp.path().join("m");
        std::fs::create_dir_all(&mdir).unwrap();
        std::fs::write(
            mdir.join(MANIFEST_FILENAME),
            r#"{"v":1,"hash_algorithm":"blake2b-256","target":"t",
                "generator_version":"g","changes":[]}"#,
        )
        .unwrap();
        let err = read_manifest(&mdir).unwrap_err();
        assert!(format!("{err}").contains("blake3-256"), "{err}");
    }
}
