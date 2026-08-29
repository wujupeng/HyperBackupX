# CR_SOURCES.md — hbx-compat-sem

## Clean-Room Implementation Source Declaration

This crate implements Duplicati configuration semantics alignment for HyperBackup X.
All implementations are based on publicly available documentation and black-box
observation. No Duplicati source code was referenced, copied, or derived.

## Reference Materials

1. **Duplicati User Manual** (https://duplicati.readthedocs.io/)
   - Filter and exclude rules syntax
   - Compression configuration options
   - Encryption configuration options
   - Backup types (full vs incremental)
   - Retention policy configuration

2. **Duplicati Configuration File Format** (publicly documented)
   - JSON-based configuration schema
   - Field names and value formats

3. **Duplicati Forum and Wiki** (publicly accessible)
   - Community-documented exception handling behavior
   - Error recovery patterns

## Implementation Approach

- **Filter Rules**: Duplicati's glob/regex/path-prefix filter syntax was mapped to
  HBX's `FilterRule` enum based on the documented wildcard semantics (`*`, `?`, `**`).
- **Version Strategy**: Duplicati's full/incremental backup modes mapped to HBX's
  `BackupType` enum based on the documented backup behavior.
- **Compression**: Duplicati's compression algorithms (zip/gzip/none) mapped to HBX's
  `CompressionAlgorithm` enum (zstd/lz4/none) based on equivalent compression categories.
- **Encryption**: Duplicati's encryption options mapped to HBX's `EncryptionProfile`
  with mandatory upgrade to AES-256-GCM + Argon2id (ADR-G1-003).
- **Metadata**: Duplicati's file metadata fields mapped to HBX's `FileEntry` struct
  with path separator normalization.
- **Exception Handling**: Duplicati's documented error recovery behavior mapped to
  HBX's `ExceptionDecision` enum based on the user manual's troubleshooting section.

## No Source Code References

- No files from the Duplicati source repository were read, copied, or adapted.
- No binary reverse engineering was performed.
- All type names, field names, and semantics are independently designed.