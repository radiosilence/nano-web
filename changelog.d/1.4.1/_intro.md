Released 2026-04-18.

### Security

- Bump `rustls-webpki` to 0.103.12 (fixes RUSTSEC-2026-0098 and RUSTSEC-2026-0099 — name constraints incorrectly accepted for URI / wildcard certs). Pulled via `reqwest` dev-dep only, so not present in shipped binaries.
- Bump `rand` to 0.9.4 (fixes RUSTSEC-2026-0097 unsoundness). Dev-dep transitive only.

### Changed

- Swap `fxhash` (unmaintained) → `rustc-hash` 2.x (maintained fork). Same algorithm, no perf impact.
- Dep bumps within existing constraints: `clap` 4.6, `hyper` 1.9, `minijinja` 2.19, `socket2` 0.6.3, `tokio` 1.52, `reqwest` 0.13, `tempfile` 3.27.
- Pinned CI action SHAs to latest releases (`dtolnay/rust-toolchain`, `Swatinem/rust-cache`).
