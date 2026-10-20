Released 2025-08-01.

### Changed

- Removed top-level serve functionality from root command
- Command now requires explicit subcommand usage

### Technical

- Simplified CLI structure by removing direct directory argument support from root command
- Eliminated RunE function from root command, forcing users to use `nano-web serve` instead of `nano-web [directory]`
