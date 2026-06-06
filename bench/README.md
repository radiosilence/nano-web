# Benchmarks

Reproducible load tests for comparing nano-web builds. Uses [`oha`](https://github.com/hatoo/oha)
for generation and JSON capture; results are diffed by `compare.py`.

## Run

```bash
bench/run.sh baseline          # build, serve, bench → bench/results/baseline/
# ...make a change...
bench/run.sh rung0             # bench the new build → bench/results/rung0/
bench/compare.py baseline rung0
```

`bench/results/` and `bench/fixtures/` are gitignored — numbers are machine- and
load-specific, so only the harness is committed, never the measurements.

## What it measures

Fixed load (env-overridable) so two runs are comparable **only on the same idle
machine with identical params**:

| Env          | Default | Meaning                    |
| ------------ | ------- | -------------------------- |
| `BENCH_CONN` | `50`    | concurrent connections     |
| `BENCH_DUR`  | `10s`   | duration per scenario      |
| `BENCH_PORT` | `38080` | server port                |
| `BENCH_BIN`  | release | binary under test          |
| `BENCH_NO_BUILD` | —   | set `1` to skip the build  |

Scenarios exercise the distinct hot paths: small HTML (identity + brotli),
compressible JS/CSS, a 1 MiB incompressible blob (bandwidth ceiling), HEAD,
404, and a 304 conditional. `compare.py` reports req/s and p50/p90/p99 latency.

## Caveat

Loopback benchmarks measure the server's per-request CPU + syscall cost, not
real network behaviour. They're for **relative** comparison between builds, not
absolute capacity claims. Kernel-bypass rungs (io_uring, etc.) are Linux-only
and must be measured on Linux, not macOS.
