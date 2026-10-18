### Fixed

- **Performance regression**: restored v1.2.0-level throughput (124k → 144k req/s) and latency (2.12ms → 345µs)
- Eliminated per-request heap allocations from security headers using `HeaderName::from_static` / `HeaderValue::from_static`
- Restored `#[inline(always)]` on hot-path functions — compiler cost model undervalues them at 150k req/s
- Pre-compute `Content-Length` header value at route creation instead of per-request integer→string conversion
- Reverted `foldhash` → `fxhash` — quality-grade random-seeded hash is unnecessary overhead for a pre-populated route cache with filesystem-derived keys
