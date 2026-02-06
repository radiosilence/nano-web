# nano-web

![CI](https://github.com/radiosilence/nano-web/actions/workflows/ci.yml/badge.svg)
![Build](https://github.com/radiosilence/nano-web/actions/workflows/build.yml/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/nano-web.svg)](https://crates.io/crates/nano-web)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

Static file server. Pre-loads and pre-compresses all files at startup for near-zero latency serving.

## Performance

- Raw hyper (no framework overhead)
- SO_REUSEPORT for multi-core scaling
- Files pre-compressed at startup (brotli/gzip/zstd)
- Lock-free concurrent routing (DashMap + FxHash)
- Zero-copy responses (Bytes)

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
FROM ghcr.io/radiosilence/nano-web:latest
COPY ./dist /public/
```

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

## Health Check

`/_health` returns `{"status":"ok","timestamp":"..."}`

## License

MIT
