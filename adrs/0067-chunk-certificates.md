---
title: Chunk Certificates
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed — deferred until untrusted chunk sources are a real concern.

  Depends on [ADR-0066 — Dependency-Graph Chunking](./0066-dependency-graph-chunking.md)
  (chunks are the unit being certified) and
  [ADR-0065 — Derived-Artifact Cache and Compression](./0065-derived-artifact-cache-and-compression.md)
  (cached artifacts are the trust targets). Refines
  [ADR-0046 — Capability Discipline at the Language Level](./0046-capability-discipline-at-the-language-level.md)
  (static capability verification) and
  [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
  (`Language` as serializable data).
---


# ADR-0067: Chunk Certificates

## Decision


### 1. A certificate is a signed statement about a chunk

A certificate is a detached signature over a canonical serialization of:

- The chunk's content hash (BLAKE3, per ADR-0065 §2).
- The chunk's dependencies (chunk ids + content hashes, per ADR-0066 §2).
- The chunk's declared capability set (per ADR-0046 §1 `CapabilitySet`).
- The chunk's `replaces` targets (per ADR-0044 §5, ADR-0066 §2).
- The chunk's `scheme_version` (per ADR-0044 §7).
- Metadata: author id, timestamp, certificate version.

```
Certificate = {
    chunk_hash: Blake3,
    dependencies: Map<ChunkId, Blake3>,
    capabilities: CapabilitySet,
    replaces: Map<FragmentId, ChunkId>,
    scheme_version: String,
    author: AuthorId,
    timestamp: u64,
    cert_version: u32,
    signature: Signature,  // over canonical_serialize of all above fields
}
```

The signature is Ed25519. The author's public key is distributed out of band
(key server, pinned in host config, or Web-of-Trust).

### 2. Verification happens before load, not after

A chunk is verified **before** it is loaded into memory. The runtime:

1. Reads the chunk and its certificate.
2. Recomputes the chunk's content hash; verifies it matches the certificate.
3. Verifies the signature against the author's public key.
4. Checks the author is in the host's trust store.
5. Checks the declared capability set against the host's policy (ADR-0046 §3).
6. Recursively verifies each dependency chunk's certificate.

If any step fails, the chunk is rejected. It is not loaded, not
decompressed, not typechecked. This is defense-in-depth: ADR-0046's
capability check inspects the data; this ADR's certificate verifies the data
hasn't been tampered with and comes from a trusted source.

### 3. Dependency certificates are required

A chunk's certificate includes its dependencies' content hashes
(ADR-0066 §2). When loading a chunk, the runtime verifies that each
dependency's actual content hash matches the hash in the certificate. This
prevents supply-chain substitution: a dependency chunk with the same id but
different content fails verification.

```
for dep_id, expected_hash in cert.dependencies:
    dep_chunk = load(dep_id)
    assert(blake3(dep_chunk) == expected_hash)
    verify_certificate(dep_chunk)
```

This is recursive: a chunk is only trusted if its entire transitive
dependency closure is trusted.

### 4. Trust store is a host config, not a language property

The set of trusted authors (public keys) is a host-level configuration, not
part of the `Language` data. A `Language` does not declare who is allowed to
author it; the host decides what to trust. This keeps trust out of the data
model (ADR-0044: `Language` is pure data, no policy) and in the host's
control (ADR-0046: the host attaches policies at registration time).

```
# host config
[trust]
trusted_authors = ["ed25519:abc123...", "ed25519:def456..."]
```

### 5. Revocation via a revocation list

A certificate can be revoked before its natural expiry. The host maintains a
revocation list (chunk hashes or author ids that are no longer trusted). A
chunk whose certificate is on the revocation list is rejected, even if the
signature is valid.

The revocation list is a host config, updated out of band (e.g., fetched
from a key server periodically). This is the standard PKI pattern; no novel
design here.

### 6. Certificates are optional per-chunk

Not every chunk needs a certificate. A chunk from a trusted local source
(in-repo, compile-time-embedded) does not need one. A chunk from an
untrusted source (third-party mod, network load, shared cache) does.

The runtime config specifies the policy:

```toml
[certificates]
required = "always" | "untrusted-only" | "never"
```

- `always` — every chunk must have a valid certificate (strictest).
- `untrusted-only` — chunks from trusted sources (local, compile-time) skip
  verification; chunks from untrusted sources (network, third-party) require
  it.
- `never` — no verification (default today; equivalent to no certificate
  subsystem).

### 7. Capability set in the certificate is a claim, verified by ADR-0046

The certificate includes the chunk's declared `CapabilitySet` (ADR-0046 §1).
This is a **claim** by the author. The runtime verifies the claim by
computing the `CapabilitySet` from the chunk's data (ADR-0046 §1
`CapabilitySet::from_language`) and comparing. A mismatch — the certificate
claims `fs-write` is not used, but the data contains a `Foreign("fs-write")`
scheme — is a verification failure. The chunk is rejected.

This closes the tampering gap: an attacker cannot modify the chunk to add a
`Foreign` scheme without invalidating the signature, and cannot lie about
the capability set without the runtime detecting the mismatch.


## Why

ADR-0065 (cache/compression) and ADR-0066 (chunking) are optimization techniques. This ADR is a different concern: **trust**. It answers "do I believe this chunk is what its author claims it is, and does it come from a source I trust?"

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Provenance and integrity, not just declared capabilities.** ADR-0046
- **Per-chunk trust.** A new chunk from a trusted author is certified
- **Supply-chain substitution is detected.** A dependency chunk with the
- **Capability-set tampering is detected.** The certificate's declared
- **Cryptographic overhead on every load of a certified chunk.** Ed25519

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0067-chunk-certificates.md`](../design/0067-chunk-certificates.md).
