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
