Released 2026-07-26.

### Changed

- **Build**: Switched from `lto = "fat"` (thin LTO) with `codegen-units = 1` to `lto = "thin"` with `codegen-units = 16`. Fat LTO with single codegen unit serializes whole-program optimization across the entire dependency tree, forcing the slowest possible build. Thin LTO with parallel codegen units cuts build time substantially. Throughput impact not measured; `bench/run.sh` + `bench/compare.py` available if needed.
