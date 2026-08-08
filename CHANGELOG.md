# Changelog

All notable changes to nano-web will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **CI**: The repository had no caches at all. GitHub evicts an entry after 7 days without a read and nano-web goes quiet for longer than that, so every run started from a cold `main` and recompiled the whole dependency graph. The workflow now also runs on a twice-weekly schedule; a run restores each cache, and a restore counts as a read.
- **CI**: `build-binaries`, `build-image` and `publish-crate` only save caches on `main`, which `test` and `lint` already did. Six branch-scoped entries per pull request — none of them readable from any other branch — were evicting `main`'s from the repository's shared 10GB.
- **CI**: `test`, `lint` and `format` are one `check` job. Clippy compiles the dependency graph and the tests reuse it, where two jobs each built it into a cache of its own.
- **CI**: Builds pass `--locked`. A resolver that is free to move produces a `Cargo.lock` that differs from the committed one, and the lockfile hash is part of the cache key.
- **CI**: `fail-fast: false` on the build matrices. A cancelled leg saves nothing, so one broken target left the rest cold on the next run too.

## [1.4.6] (2026-07-27)

### Fixed

- **Dev mode**: `--dev` now reloads the URLs people actually load. Index files are published under two keys — `/index.html` and `/` — but the refresh only re-inserted the canonical one, so editing `index.html` and reloading `/` served the bytes captured at startup, indefinitely. Route publishing now goes through one function shared by the startup scan and the refresh, so the alias cannot be left behind again.
- **Dev mode**: Files created after startup are picked up instead of 404ing forever. The refresh returned early when a URL had no route, which is exactly the case for a new file.
- **Dev mode**: Deleted files stop being served instead of returning their last-known contents. The missing-file error was discarded by the caller, leaving the stale route in place.

Production serving is unaffected — routes there are write-once by design.

## [1.4.5] (2026-07-26)

### Changed

- **Build**: Restored `lto = "fat"` with `codegen-units = 1`. 1.4.4 moved to thin LTO as part of a fleet-wide sweep; that is right for the I/O-bound MCP services, which spend their time blocked on sockets, but wrong here. nano-web is CPU-bound on its hot path and is meant to be as optimised as possible, so the slower build is the intended trade. `bench/run.sh` + `bench/compare.py` are there if the actual delta is ever worth measuring.

## [1.4.4] (2026-07-26)

### Changed

- **Build**: Switched from `lto = "fat"` (thin LTO) with `codegen-units = 1` to `lto = "thin"` with `codegen-units = 16`. Fat LTO with single codegen unit serializes whole-program optimization across the entire dependency tree, forcing the slowest possible build. Thin LTO with parallel codegen units cuts build time substantially. Throughput impact not measured; `bench/run.sh` + `bench/compare.py` available if needed.

## [1.4.3] (2026-07-26)

### Changed

- **Docker**: The image has no build stage at all now — it is a single `FROM scratch` that copies in the musl binary CI already built. Building it by hand requires `dist/` populated first; docker builds only ever happen in CI, so the source-build path was carrying no weight.
- **CI**: Docker images no longer recompile the binary. `build-image` now owns the linux musl compile and copies the result straight into a `scratch` image on the same runner, then uploads it as the release asset — removing two full `lto = "fat"` compiles per push.
- **CI**: Dropped the GHA layer cache on the Docker builds. With no compile in the image there is nothing worth caching, and `mode=max` was filling the repo-wide 10GB cache that `Swatinem/rust-cache` shares.
- **CI**: `test` and `lint` only save caches on `main`. Branch-scoped caches can't be read by other branches, so PR runs were writing entries that only evicted the ones they restore from.
- **CI**: Dropped the redundant cache on the formatting job — `cargo fmt --check` compiles nothing.
- **CI**: PRs now run the full build chain — every target binary plus both Docker images. Only the registry push, manifest, release and crate publish stay gated on `main`. A broken `Dockerfile` or a target that stops compiling now fails before merge instead of after.

### Removed

- **CI**: The `quick-test` Docker smoke test job. Release now gates on `create-manifest` so the tag still lands only after the images are pushed.

## [1.4.2] (2026-04-18)

### Fixed

- **CI**: `cargo publish` now uses `--locked` (prevents lockfile regen from failing the publish with "working directory contains changes"). Also switched from deprecated `--token` flag to `CARGO_REGISTRY_TOKEN` env var. v1.4.1 shipped to GitHub / Docker but not crates.io — v1.4.2 is otherwise identical and is the first to land on crates.io in the 1.4.x line.

## [1.4.1] (2026-04-18)

### Security

- Bump `rustls-webpki` to 0.103.12 (fixes RUSTSEC-2026-0098 and RUSTSEC-2026-0099 — name constraints incorrectly accepted for URI / wildcard certs). Pulled via `reqwest` dev-dep only, so not present in shipped binaries.
- Bump `rand` to 0.9.4 (fixes RUSTSEC-2026-0097 unsoundness). Dev-dep transitive only.

### Changed

- Swap `fxhash` (unmaintained) → `rustc-hash` 2.x (maintained fork). Same algorithm, no perf impact.
- Dep bumps within existing constraints: `clap` 4.6, `hyper` 1.9, `minijinja` 2.19, `socket2` 0.6.3, `tokio` 1.52, `reqwest` 0.13, `tempfile` 3.27.
- Pinned CI action SHAs to latest releases (`dtolnay/rust-toolchain`, `Swatinem/rust-cache`).

## [1.4.0] (2026-03-26)

### Fixed

- **Security**: use sanitized path from `validate_request_path` for all route lookups instead of raw request URI — prevents potential path-based bypasses
- **Correctness**: ETag now uses content hash (FxHash) instead of timestamp+length — deterministic across restarts, correct for identical content
- **Correctness**: `Encoding::from_accept_encoding` now respects `q=0` (encoding explicitly rejected by client)
- **Perf**: added `application/javascript` to compressible types — `mime_guess` returns this for `.js` files, so JS was silently not being compressed
- **Correctness**: `Vary: Accept-Encoding` now sent on all compressible content types, not just compressed responses — fixes CDN/proxy cache poisoning

### Added

- Graceful shutdown via `tokio::signal` — handles SIGTERM and SIGINT for clean container stops

### Changed

- Replaced `.unwrap()` on response builders with `.expect()` for better panic diagnostics
- Removed unused `CachedRoute.content` field — compressed data was duplicated between route cache and response cache
- Removed unused HTTP/2 feature flags from `hyper` and `hyper-util` dependencies
- Removed cargo-cult `#[inline(always)]` attributes — let the compiler decide

## [1.3.1]

### Fixed

- **Performance regression**: restored v1.2.0-level throughput (124k → 144k req/s) and latency (2.12ms → 345µs)
- Eliminated per-request heap allocations from security headers using `HeaderName::from_static` / `HeaderValue::from_static`
- Restored `#[inline(always)]` on hot-path functions — compiler cost model undervalues them at 150k req/s
- Pre-compute `Content-Length` header value at route creation instead of per-request integer→string conversion
- Reverted `foldhash` → `fxhash` — quality-grade random-seeded hash is unnecessary overhead for a pre-populated route cache with filesystem-derived keys

## [1.3.0]

### Changed

- Zero-copy response buffers — `ResponseBuffer` now accepts `Bytes` directly, eliminating redundant `.to_vec()` copies on every compressed response
- Replaced `chrono` with `httpdate` — `chrono` was overkill for HTTP date formatting, `httpdate` is purpose-built and tiny. **Note:** `/_health` timestamp format changed from RFC 3339 to HTTP-date
- Removed redundant `FinalServeConfig` intermediate type in CLI
- Strict clippy pedantic lints enabled via `[lints.clippy]` in `Cargo.toml`
- `is_compressible()` simplified from match-with-identical-arms to `matches!` macro
- `handle_request` no longer needlessly `async` — sync function returning `Result` is sufficient for hyper's `service_fn`

### Added

- Security headers: `Strict-Transport-Security`, `Permissions-Policy`, `X-DNS-Prefetch-Control`
- `Vary: Accept-Encoding` header on compressed responses (correct HTTP caching semantics)
- `Content-Length` header on all responses (pre-computed at route creation, zero per-request cost)
- MSRV 1.75 declared in `Cargo.toml`
- Integration tests for new security headers, cache-control values, content-length, vary header
- Dynamic port allocation in tests (no more hardcoded port conflicts)

### Fixed

- **Performance**: eliminated per-request heap allocations from security headers by using `HeaderName::from_static` / `HeaderValue::from_static` for all constant headers
- **Performance**: restored `#[inline(always)]` on hot-path functions (`get_response`, `get_map`, `get`, `build_response`, `from_accept_encoding`) — the compiler's cost model underestimates their value at 150k req/s
- **Performance**: pre-compute `Content-Length` header value at route creation instead of integer→string conversion per request
- Pedantic clippy warnings: uninlined format args, `map`/`unwrap_or_else` patterns, match-for-single-pattern, doc backticks

## [1.2.0]

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
