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
