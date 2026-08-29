# CR_SOURCES.md — hbx-compat-engine

## Clean-Room Source Declaration

This crate implements the **compatibility backup/restore engine** for HyperBackup X,
enabling Duplicati-semantic backup operations through an adapter pattern that reuses
the existing HBX pipeline infrastructure.

## Implementation Approach

### Adapter Pattern (Zero-Copy Reuse)

The `CompatibilityRepoAdapter` wraps `CompatibleRepository` and implements the
existing `IBackupRepository` trait. This allows the full HBX pipeline
(Scanner → Chunker → Dedup → Compressor → Encryptor) to operate unchanged,
with only the repository backend replaced.

**No pipeline code was copied or modified.** The adapter performs type conversions
between native HBX domain types (`Manifest`, `FileEntry`, `EncryptedChunk`) and
compatibility repository types (`CompatibilityManifest`, `CompatFileEntry`).

### Semantic Alignment

The engine accepts `DuplicatiConfig` (Duplicati-style configuration) and is designed
to inject `ISemanticAligner` from `hbx-compat-sem` to map Duplicati semantics to
HBX internal semantics before pipeline execution.

### Design Sources

| Source | Type | Usage |
|--------|------|-------|
| HBX existing pipeline architecture | Original work | Reused via adapter pattern |
| Duplicati User Manual (online) | Public documentation | Configuration format semantics |
| Duplicati Forum posts (public) | Community discussion | Backup/restore workflow semantics |

## No Source Code References

- No Duplicati source files were read during development
- No Duplicati binary was disassembled or decompiled
- All code in this crate is original work by the HyperBackup X development team
- The adapter pattern ensures full reuse of existing HBX pipeline without duplication