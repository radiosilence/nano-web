Released 2025-07-31.

### Changed

- Migrated CLI framework from Kong to Cobra
- Restructured command architecture with explicit subcommands

### Added

- Shell completion support for bash, zsh, fish, and powershell
- `completion` subcommand for generating shell completions
- Enhanced flag completion with directory suggestions
- Proper command structure with `serve` and `version` subcommands

### Technical

- Replaced alecthomas/kong dependency with spf13/cobra
- Refactored ServeCmd struct to ServeConfig for better separation
- Added comprehensive flag validation and completion functions
- Improved environment variable handling with helper functions
