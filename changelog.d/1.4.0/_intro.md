Released 2026-03-26.

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
