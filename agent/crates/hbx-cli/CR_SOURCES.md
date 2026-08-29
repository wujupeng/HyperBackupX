# CR_SOURCES.md - hbx-cli

## Clean-room Implementation Source Declaration

### Module
`hbx-cli` - Command-line interface for HyperBackup X

### Implementation Sources
1. **CLI argument parsing**: Self-implemented using `std::env::args` iterator pattern.
   No external CLI framework (clap) was available, so a minimal argument parser
   was clean-room implemented based on standard Unix CLI conventions.

2. **Command structure**: Inspired by common backup tool CLI patterns
   (backup/restore/list/delete/verify/import subcommands).
   No specific source referenced; design is original.

3. **HTTP client**: Uses `ureq` crate for REST API communication.
   API endpoints are defined by HBX Control Plane's REST API specification.

4. **Configuration**: TOML-based configuration file format,
   self-designed for HBX agent configuration.

### No Duplicati Source Code Referenced
This module does not reference, copy, or derive from any Duplicati source code.
The CLI design is based on standard command-line interface conventions.

### Verification
- All code is original work by the HBX development team
- No comments, variable names, or structures trace back to Duplicati source
- Implementation based on public documentation and standard CLI patterns