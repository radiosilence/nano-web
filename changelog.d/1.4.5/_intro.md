Released 2026-07-26.

### Changed

- **Build**: Restored `lto = "fat"` with `codegen-units = 1`. 1.4.4 moved to thin LTO as part of a fleet-wide sweep; that is right for the I/O-bound MCP services, which spend their time blocked on sockets, but wrong here. nano-web is CPU-bound on its hot path and is meant to be as optimised as possible, so the slower build is the intended trade. `bench/run.sh` + `bench/compare.py` are there if the actual delta is ever worth measuring.
