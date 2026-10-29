---
title: Chunk Certificates
type: adr
date: 2026-09-02
status: proposed
---

# ADR-0067: Chunk Certificates

## Status

Proposed — deferred until untrusted chunk sources are a real concern.

Depends on [ADR-0066 — Dependency-Graph Chunking](./0066-dependency-graph-chunking.md)
(chunks are the unit being certified) and
[ADR-0065 — Derived-Artifact Cache and Compression](./0065-derived-artifact-cache-and-compression.md)
(cached artifacts are the trust targets). Refines
[ADR-0046 — Capability Discipline at the Language Level](./0046-capability-discipline-at-the-language-level.md)
(static capability verification) and
[ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
(`Language` as serializable data).

## Context

### This is a trust/provenance feature, not an optimization

ADR-0065 (cache/compression) and ADR-0066 (chunking) are optimization
techniques. This ADR is a different concern: **trust**. It answers "do I
believe this chunk is what its author claims it is, and does it come from a
source I trust?"

The distinction matters because bundling trust under "optimization" obscures
the threat model. Optimization is about performance; trust is about safety.
They share a unit (the chunk, per ADR-0066) but they have different failure
modes, different stakeholders, and different design constraints. This ADR
keeps them separate.

### The threat model

ADR-0046 made capability discipline static: a `Language`'s capability set is
computed from its data, and `eval` checks it at the boundary. This prevents
a `Language` from exercising capabilities it doesn't declare. It does **not**
prevent:

1. **Tampering.** A chunk is modified after authoring (by an attacker, a
   corrupted cache, a MITM on a network load) to declare fewer capabilities
   than it actually exercises, or to replace a safe fragment with a
   malicious one.
2. **Misattribution.** A chunk claims to be from a trusted author but isn't.
3. **Supply-chain substitution.** A chunk dependency (ADR-0066 §2
   `dependencies: Set<ChunkId>`) is replaced with a different chunk that
   has the same id but different content.

ADR-0046's capability check inspects the `Language` data as presented. If the
data has been tampered with to lie about its capabilities, the check passes
falsely. Certificates close this gap by verifying **provenance and
integrity**, not just declared capabilities.

### Why this depends on ADR-0066

Chunks (ADR-0066) are the unit of certification. A certificate covers a
chunk: its content, its dependencies, its author, its capability set. Without
chunking, the unit is the whole `Language` — too coarse for incremental
trust (one changed fragment re-certifies the entire language). With chunking,
trust is per-chunk: a new chunk from a trusted author is certified
independently; an untrusted chunk is rejected without loading the rest.

### Why this is deferred

Today, all `Language` data is authored in-repo and embedded at compile time
(ADR-0044 §6). There are no untrusted sources. Certificates add overhead
(signature verification on every load, key management, revocation) with no
benefit when the source is trusted by construction.

Certificates pay off when:

- **Mods/plugins from third parties.** A user loads a mod's `Language`
  chunks; the host verifies the mod author's signature before eval.
- **Network-loaded languages.** Chunks fetched over a network can be
  tampered with in transit; signatures verify integrity.
- **Shared caches (ADR-0065 §5 `NetworkCache`).** A cache entry from another
  host must be verified before use — the cache is not trusted.
- **Compliance/regulatory environments.** A deployment requires provenance
  attestation for all loaded code.

None of these drivers exist today. This ADR records the design — because
trust is a distinct concern from optimization and should not be designed ad
hoc when the first untrusted source appears — but marks it **deferred until
a concrete driver exists**.

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

## Consequences

### Positive

- **Provenance and integrity, not just declared capabilities.** ADR-0046
  checks what the data claims; this ADR verifies the data hasn't been
  tampered with and comes from a trusted author. Together they form
  defense-in-depth.
- **Per-chunk trust.** A new chunk from a trusted author is certified
  independently; an untrusted chunk is rejected without loading the rest.
  This composes with ADR-0066's lazy load.
- **Supply-chain substitution is detected.** A dependency chunk with the
  same id but different content fails the hash check in the parent's
  certificate.
- **Capability-set tampering is detected.** The certificate's declared
  capability set is verified against the data; a mismatch is a rejection.

### Negative

- **Cryptographic overhead on every load of a certified chunk.** Ed25519
  verification is fast (~100µs) but non-zero. For a chunk loaded on every
  eval, this adds up. Mitigation: verify once, cache the verification
  result alongside the chunk (ADR-0065 cache stores "verified" flag).
- **Key management is a real operational burden.** Key distribution,
  rotation, revocation, trust store management — this is PKI, not a
  wrapper. The host operator bears this cost.
- **Revocation lists can grow.** A long-lived deployment accumulates
  revoked chunks. The list must be pruned or bounded.
- **A compromised author key compromises all chunks signed by it.**
  Standard PKI failure mode; revocation is the mitigation, but there is a
  window between compromise and revocation.

### Neutral

- **Deferred.** This ADR records the design but does not mandate
  implementation. A concrete driver (third-party mods, network-loaded
  languages, shared caches, compliance requirements) must appear before
  implementation begins.
- **Separate from optimization.** Certificates are not a performance
  feature. They are a trust feature that happens to share a unit (chunk)
  with the optimization features (ADR-0065, ADR-0066).

## Implementation phases (deferred)

### Phase 1 — Certificate format and signing

Define the `Certificate` struct, canonical serialization, and Ed25519
signing/verification. Test: sign a chunk, verify the signature; tamper the
chunk, verification fails.

### Phase 2 — Trust store and verification pipeline

Implement the trust store (host config), the verification pipeline (hash →
signature → author → capabilities → dependencies), and the revocation list.
Test: a chunk from a trusted author loads; a chunk from an untrusted author
is rejected; a revoked chunk is rejected.

### Phase 3 — Integration with chunk loading (ADR-0066)

Wire certificate verification into the chunk-load path (ADR-0066 §3). A
chunk is verified before decompression and typecheck. Test: the lazy-load
pipeline verifies each chunk on demand.

### Phase 4 — Capability-set mismatch detection

Verify the certificate's declared `CapabilitySet` against the computed one
(ADR-0046 §1). Test: a chunk whose data contains a `Foreign` not in the
certificate's capability set is rejected.

## References

- [ADR-0066](./0066-dependency-graph-chunking.md) — Dependency-Graph
  Chunking (chunks are the unit of certification)
- [ADR-0065](./0065-derived-artifact-cache-and-compression.md) —
  Derived-Artifact Cache and Compression (cached artifacts are trust
  targets; verification result can be cached)
- [ADR-0046](./0046-capability-discipline-at-the-language-level.md) —
  Capability Discipline (static capability check; this ADR verifies the
  data the check inspects)
- [ADR-0044](./0044-language-as-data-and-runtime-split.md) — Language as Data
  and Runtime Split (`Language` is serializable data; certificates sign
  that data)
- Ed25519 (RFC 8032) — the signature scheme
- Standard PKI patterns (X.509, TUF, Sigstore) — key distribution,
  revocation, trust store design
