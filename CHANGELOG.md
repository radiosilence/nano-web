# Changelog

All notable changes to nano-web will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.7]

### Fixed

- Accept-Encoding parsing: substring false positives (e.g. "br" matching "vibrant") now properly tokenized
- HEAD requests now return empty body with correct headers per HTTP spec
- ETag conditional requests: return 304 Not Modified when `If-None-Match` matches
- `--log-requests` flag now actually logs requests (was stored but never used)
- VERSION file synced with Cargo.toml

### Removed

- Dead code: unused `ResponseBuffer::not_found()`/`bad_request()` static error responses
- Dead code: unused `CompressedContent::get_best_encoding()` method (clippy errors)
- Dead code: unused `CachedRouteHeaders` struct (headers already baked into ResponseBuffer)
- Unnecessary dependencies: `ahash` (unused), `md5` (unused), `cargo-release` (CLI tool, not a lib dep)
- Unnecessary feature flags: `chrono/serde`, `serde/derive` (neither used)
- Stale comment referencing removed Axum implementation

### Changed

- Tightened visibility: `CachedRoute`, `CachedRouteHeaders`, `CachedRoutes`, `NanoWeb.routes` are now private
- Made stateless methods (`generate_etag`, `format_http_date`, `file_path_to_url`) associated functions
- `env::set_var`/`env::remove_var` calls in tests wrapped in `unsafe` blocks with safety comments (required since Rust 1.66)

### Added

- Unit tests for `Encoding::from_accept_encoding` (priority, substring safety, quality values)
- Integration tests for HEAD requests, ETag/304, and METHOD_NOT_ALLOWED

## [1.1.5]

- Fixed non-compressible files (images, etc) returning 404 when Accept-Encoding header present
- Zero-allocation response lookups (split response cache by encoding)
- Static error responses (404/400 no longer allocate)
- Fixed dev mode file reloading
- Fixed status codes for 404 and 400 responses
- Consistent encoding priority (br > zstd > gzip)
- Code cleanup

## [1.1.2]

- Add musl builds
- Use mimalloc for docker/musl builds
- Bump dependencies

## [1.0.7]

- Maximum compression for various libs
- Code reorganisation a bit

## [1.0.5] - 2025-08-07

### Technical

- dont enable cpu native

## [0.12.1] - 2025-08-01

### Added

- Add automated changelog system with complete version history

## [0.12.0] - 2025-08-01

### Changed

- Removed top-level serve functionality from root command
- Command now requires explicit subcommand usage

### Technical

- Simplified CLI structure by removing direct directory argument support from root command
- Eliminated RunE function from root command, forcing users to use `nano-web serve` instead of `nano-web [directory]`

## [0.11.0] - 2025-08-01

### Changed

- Default port changed from 80 to 3000
- Streamlined release process with semantic versioning and automated Homebrew updates

### Technical

- Added DefaultPort constant set to 3000
- Enhanced GitHub Actions workflow for automated releases
- Updated Dockerfile to use new default port
- Improved Taskfile.yml configuration

## [0.10.1] - 2025-08-01

### Technical

- Updated build configuration and documentation
- Refined Taskfile.yml and README structure

## [0.10.0] - 2025-08-01

### Technical

- Version bump without functional changes

## [0.9.1] - 2025-07-31

### Technical

- Version bump without functional changes

## [0.9.0] - 2025-07-31

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

## [0.8.1] - 2025-06-02

### Added

- Homebrew installation support
- Pre-built binaries in Homebrew formula

### Technical

- Added Formula/nano-web.rb for Homebrew distribution
- Optimized Docker builds with native ARM64 runners
- Added HOMEBREW.md installation guide

## [0.8.0] - 2025-05-31

### Added

- Zstandard (zstd) compression support alongside existing gzip and brotli

### Technical

- Enhanced compression.go with zstd encoding capability
- Extended compression tests for all three formats
- Updated route compression logic to support zstd

## [0.7.9] - 2025-05-30

### Technical

- Version bump without functional changes

## [0.7.2] - 2025-05-30

### Technical

- Version bump and merge cleanup

## [0.7.1] - 2025-05-30

### Technical

- Version bump without functional changes

## [0.7.0] - 2025-05-30

### Added

- Aggressive HTTP cache headers with intelligent cache control
- Asset-specific caching strategies (1 year for assets, 15 minutes for HTML, 1 hour for other content)

### Technical

- Added CacheControl header to Routes struct
- Implemented getCacheControl function with MIME type-based caching rules
- Enhanced isAsset detection for CSS, JavaScript, images, fonts, audio, and video
- Removed benchmark pipeline and updated CI configuration
- Improved README with badges and cleaner documentation

## [0.6.2] - 2025-05-27

### Fixed

- Corrected environment variable flag name for dev mode

### Technical

- Fixed dev environment flag handling

## [0.6.1] - 2025-05-27

### Fixed

- Regression where environment variables were being ignored
- Removed health check CLI functionality

### Technical

- Restored proper environment variable integration
- Cleaned up CLI structure and documentation

## [0.6.0] - 2025-05-27

### Added

- Development mode with automatic file change detection and reloading
- Real-time file modification monitoring

### Changed

- Enhanced version output with fire emoji

### Technical

- Added Dev flag to ServeCmd struct
- Implemented file modification time checking in dev mode
- Added automatic route recaching when files are modified
- Enhanced route structure with Path, ModTime, and Headers fields
- Improved ETag generation using MD5 hashing of modification timestamps
- Added comprehensive file stat checking and error handling

## [0.5.10] - 2025-05-27

### Fixed

- Removed problematic double slash route handling
- Improved JavaScript injection examples

### Technical

- Code formatting improvements with gofmt
- Better route path normalization

## [0.5.9] - 2025-05-27

### Technical

- Cleaned up source references and documentation

## [0.5.8] - 2025-05-27

### Technical

- Version bump without functional changes

## [0.5.7] - 2025-05-27

### Technical

- Moved source files to root directory structure

## [0.5.6] - 2025-05-27

### Technical

- Version bump without functional changes

## [0.5.5] - 2025-05-27

### Technical

- Fixed VERSION file format

## [0.5.4] - 2025-05-27

### Technical

- Version bump without functional changes

## [0.5.3] - 2025-05-27

### Technical

- Version bump without functional changes

## [0.5.2] - 2025-05-27

### Technical

- Version bump without functional changes

## [0.5.1] - 2025-05-27

### Technical

- Cleaned up old main file references

## [0.5.0] - 2025-05-27

### Changed

- Major code restructuring and refactoring
- Moved source files to dedicated src/ directory structure

### Fixed

- Corrected default port configuration
- Resolved refactoring-related issues

### Technical

- Split monolithic main.go into modular components: compression.go, routes.go, server.go, template.go, mimetypes.go
- Enhanced MIME type handling with comprehensive type definitions
- Improved compression logic separation
- Better code organization and maintainability
- Updated Dockerfile and Taskfile.yml for new structure
