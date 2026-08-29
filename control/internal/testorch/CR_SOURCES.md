# CR_SOURCES.md - control/internal/testorch

## Clean-room Implementation Source Declaration

### Module
`control/internal/testorch` - Test orchestration infrastructure

### Implementation Sources
1. **Matrix executor (L1-L5)**: Compatibility matrix definition and execution
   - Self-designed based on HBX-G1 specification §5.1-5.5
   - 125 matrix entries covering L1-L5 compatibility layers
   - No Duplicati source code referenced

2. **Golden test set**: 1000 reference test scenarios
   - Self-designed based on HBX-G1 specification §5.9
   - Scenario categories: backup/restore/verify/retention/exception
   - No Duplicati source code referenced

3. **Dual-run comparator**: Side-by-side HBX vs Duplicati comparison
   - Based on HBX-G1 design document §2.4.3
   - Comparison dimensions: SHA-256, directory tree, file size, metadata
   - Original implementation

4. **Fuzz testing**: Perturbation generator and pipeline runner
   - Based on HBX-G1 specification §5.11
   - 10 perturbation types, 6-stage pipeline
   - Original implementation

5. **Chaos testing**: Fault injection and damage detection
   - Based on HBX-G1 specification §5.12
   - 5 fault types, damage detection + recovery rejection
   - Original implementation

6. **Acceptance**: Six-line conclusion and signing gate
   - Based on HBX-G1 specification §5.13
   - Six verification lines with evidence chain
   - Original implementation

### No Duplicati Source Code Referenced
This module does not reference, copy, or derive from any Duplicati source code.
All test scenarios and infrastructure are original work.

### Verification
- All code is original work by the HBX development team
- Implementation based on HBX-G1 specification and design documents
- No comments, variable names, or structures trace back to Duplicati source