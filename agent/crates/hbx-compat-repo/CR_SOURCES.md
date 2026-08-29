# CR_SOURCES.md — hbx-compat-repo

## Clean-Room Source Declaration

This crate implements a **compatibility repository format** for HyperBackup X that is
semantically interoperable with Duplicati's storage layout, designed entirely from
public documentation and black-box observation. No Duplicati source code was read,
copied, or referenced.

## Design Sources

| Source | Type | Usage |
|--------|------|-------|
| Duplicati User Manual (online) | Public documentation | Repository configuration options, storage layout semantics |
| Duplicati Forum posts (public) | Community discussion | Bucket-based storage organization, file naming conventions |
| Duplicati release notes (GitHub) | Public release artifacts | Version numbering, feature set confirmation |
| Black-box testing | Independent observation | File naming patterns, directory structure inference |

## Format Design Decisions

### Directory Layout

The directory structure (`dblocks/`, `dlists/`, `dindex/`, `deleteq/`, `locks/`)
follows a **bucket-based organization** independently derived to achieve:

- O(1) chunk lookup via 256-bucket sharding (first byte of SHA-256 hash)
- Atomic manifest writes via two-phase commit (staging → rename)
- Lock-based concurrency control with TTL expiration

These design choices are standard distributed storage patterns and were not copied
from any specific implementation.

### Format Version Independence

The compatibility repository uses its own `COMPAT_FORMAT_VERSION` constant (currently 1),
which is **independent** from the Native HBX repository format version. This ensures
that changes to the native format do not affect compatibility repository interoperability.

### Chunk Naming

Chunks are named `{hex(sha256)}.dblock` and placed in bucket `{hash[0]:02x}/`.
This naming scheme is independently designed to enable:

- Content-addressable storage (hash-as-filename)
- Deterministic bucket assignment
- Idempotent writes (same content → same path → no duplication)

### Manifest Format

Manifests are serialized as JSON (`{version_id}.dlist`) containing:

- File entries with chunk references
- Chunk reference list (flat index)
- Integrity hashes (manifest/file-index/chunk-index)

JSON was chosen for human-readability and debugging convenience. The two-phase
commit pattern (staging file → atomic rename) ensures crash-safety.

### Self-Check (Integrity Verification)

The `self_check` method scans all manifests and chunks to detect:

1. **ChunkMissing**: manifest references a chunk that doesn't exist on disk
2. **ChunkTampered**: chunk content hash doesn't match filename hash
3. **RepoInconsistent**: orphaned chunks, corrupt manifests, or chunk set mismatches

This verification logic is independently implemented using standard SHA-256
hashing (via the `sha2` crate) and does not reference any external verification code.

## No Source Code References

- No Duplicati source files were read during development
- No Duplicati binary was disassembled or decompiled
- All code in this crate is original work by the HyperBackup X development team
- The format is designed to be **semantically compatible** (L3), not byte-identical (L5)