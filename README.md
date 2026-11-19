# nano-web

![CI](https://github.com/radiosilence/nano-web/actions/workflows/ci-cd.yml/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/nano-web.svg)](https://crates.io/crates/nano-web)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

Static file server. Pre-loads and pre-compresses all files at startup for near-zero latency serving.

## Performance

- Raw hyper
- SO_REUSEPORT for multi-core scaling
- Files pre-compressed at startup (brotli/gzip/zstd)
- Lock-free concurrent routing (DashMap + rustc-hash)
- Zero-copy responses (Bytes), with the full header block precomputed per route

Benchmark methodology and a reproducible harness live in [`bench/`](bench/).

Benchmark (M3 Max):

```
wrk -c 50 -d 10 -t 50 http://localhost:3000
Requests/sec: 149838.48
Latency: 328.63us avg
```

## Install

```bash
# mise
mise install ubi:radiosilence/nano-web

# cargo
cargo install nano-web
```

Pre-built binaries on [GitHub Releases](https://github.com/radiosilence/nano-web/releases).

## Docker

```dockerfile
# syntax=docker/dockerfile:1
FROM ghcr.io/radiosilence/nano-web:latest
COPY --chmod=a+rX ./dist /public/
```

The image runs as `nobody` (65534), so the bundle has to be world-readable:
every file readable, and every directory readable and searchable. Nothing fails
loudly otherwise. A file nobody can read is skipped when the bundle loads and
answers 404, and a directory nobody can read takes its whole subtree with it,
without a log line. `COPY` keeps the source's modes, so `--chmod` guards against
a build context or builder stage with restrictive ones; the `syntax` line gives
older builders a frontend that accepts a symbolic mode. A bundle bind-mounted
at `/public` instead needs the same modes on the host, or `--user` set to a uid
that can read it.

The default port, 3000, needs no privilege. A `--port` below 1024 works where
the runtime allows unprivileged binds, as Docker and containerd 2 do by
default. Host networking and some older Kubernetes runtimes do not; there, set
the `net.ipv4.ip_unprivileged_port_start` pod sysctl. `USER 0` in a derived
image restores the old behaviour.

## Usage

```bash
nano-web serve                          # ./public on :3000
nano-web serve ./dist --port 8080       # custom dir/port
nano-web serve --spa --dev              # SPA mode + hot reload
```

## Options

| Flag              | Default   | Description                           |
| ----------------- | --------- | ------------------------------------- |
| `--port`, `-p`    | `3000`    | Port                                  |
| `--spa`           | `false`   | Serve index.html for unknown routes   |
| `--dev`, `-d`     | `false`   | Reload modified files                 |
| `--config-prefix` | `VITE_`   | Env var prefix for template injection |
| `--log-level`     | `info`    | debug/info/warn/error                 |
| `--log-format`    | `console` | console/json                          |
| `--log-requests`  | `false`   | Log each request                      |

## Runtime Config Injection

Inject env vars into HTML at startup:

```html
<script>
  window.ENV = JSON.parse("{{EscapedJson}}");
</script>
```

Variables: `{{env.VAR_NAME}}`, `{{Json}}`, `{{EscapedJson}}`

## Caching

ETag and `If-None-Match` supported - returns 304 Not Modified when content hasn't changed.

## Security Headers

All responses include: `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `Strict-Transport-Security`, `Permissions-Policy`, `X-DNS-Prefetch-Control`. Compressed responses include `Vary: Accept-Encoding`.

## Health Check

`/_health` returns `{"status":"ok","timestamp":"..."}`

## License

MIT
