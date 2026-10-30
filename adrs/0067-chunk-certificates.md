---
title: Chunk Certificates
type: adr
date: 2026-09-02
status: proposed
status_note: Accepted — a certificate is a signed statement, verified before load.
---

# ADR-0067: Chunk Certificates

## Decision

1. **A certificate is an Ed25519 detached signature** over the canonical
   serialization of: chunk content hash (BLAKE3), dependency hashes, capability
   set, `replaces` targets, `scheme_version`, author id, timestamp and
   certificate version.
2. **Verification happens before load**: recompute the hash, verify the
   signature, check the author in the trust store, check the capability policy
   (ADR-0046), then verify every dependency's certificate. A failure means the
   chunk is never loaded, decompressed or typechecked.
3. **Dependency certificates are required** and pin each dependency's content
   hash; a same-id/different-content dependency fails, closing supply-chain
   substitution.
4. **Author keys are distributed out of band** — key server, pinned host
   config, or Web-of-Trust; the certificate carries the author id, not the key.
5. **Certificates complement capability checks**: ADR-0046 inspects what the
   data may do; this verifies the data is intact and from a trusted author.

## Why

Content hashes proved identity but not provenance: anyone could publish a
chunk with a matching id and different content, and capability checks cannot
tell tampering from intent. A signed statement over hash + deps + capabilities
makes substitution detectable before any code runs.

## Not doing
- **Verify after load** — too late; the chunk must be rejected first.
- **Trust on first use** — authors are in a trust store, not learned.
- **Skip dependency verification** — a chain is only as strong as its edges.

## Consequences
- Loading is gated on signature + trust + capabilities.
- Authors must publish keys; hosts must maintain a trust store.
- Dependency hashes become part of the certificate, so substitution fails.

## Enforcement
- `handl-pkg-vendor/` — certificate creation and verification.
- `handl-runtime/` — pre-load verification path.
- Tests for tampered chunks, untrusted authors and substituted deps.

## Design
+The certificate shape, verification order and dependency rules:
+[`design/0067-chunk-certificates.md`](../design/0067-chunk-certificates.md).
