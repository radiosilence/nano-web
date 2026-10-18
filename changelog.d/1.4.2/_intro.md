Released 2026-04-18.

### Fixed

- **CI**: `cargo publish` now uses `--locked` (prevents lockfile regen from failing the publish with "working directory contains changes"). Also switched from deprecated `--token` flag to `CARGO_REGISTRY_TOKEN` env var. v1.4.1 shipped to GitHub / Docker but not crates.io — v1.4.2 is otherwise identical and is the first to land on crates.io in the 1.4.x line.
