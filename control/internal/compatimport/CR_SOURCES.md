# CR_SOURCES.md - control/internal/compatimport

## Clean-room Implementation Source Declaration

### Module
`control/internal/compatimport` - Duplicati configuration import

### Implementation Sources
1. **Duplicati config parsing**: Based on Duplicati's publicly documented
   JSON configuration format (Duplicati JSON export format).
   - Source: Duplicati official documentation (https://duplicati.readthedocs.io/)
   - The parser reads JSON configuration files and maps fields to HBX equivalents.
   - No Duplicati source code was referenced; only the public JSON schema.

2. **Field mapping**: Duplicati field names → HBX field names
   - Mapping table created from Duplicati's public documentation
   - Example: "EncryptionModule" → "encryption_algorithm"
   - No source code copied; only field name documentation used

3. **Unsupported item handling**: Items not yet supported are reported, not silently dropped
   - Original design decision for transparency

### No Duplicati Source Code Referenced
This module parses Duplicati's publicly documented JSON configuration format.
No Duplicati source code was referenced, copied, or derived from.

### Verification
- All code is original work by the HBX development team
- Configuration format based on Duplicati's public documentation
- No comments, variable names, or structures trace back to Duplicati source