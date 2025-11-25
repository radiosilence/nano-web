# nano-web

![CI](https://github.com/radiosilence/nano-web/actions/workflows/ci.yml/badge.svg)
![Build](https://github.com/radiosilence/nano-web/actions/workflows/build.yml/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/nano-web.svg)](https://crates.io/crates/nano-web)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

Static file server. Pre-loads and pre-compresses everything at startup for zero-latency serving.

## Why

Most static file servers read from disk on every request, relying on OS page cache. This works fine until you need consistent low latency under load. nano-web loads everything into memory at startup with pre-computed compressed variants, so serving a file is just a hashmap lookup and a buffer copy.

## Performance

~150k req/sec on M3 Max, sub-millisecond latency. The server does almost nothing per-request by design.

```
wrk -c 50 -d 10 -t 50 http://localhost:3000
  Latency   328.63us   47.98us   2.86ms   88.05%
  Req/Sec     3.01k   103.63     3.21k    91.58%
  1513328 requests in 10.10s, 8.58GB read
Requests/sec: 149838.48
```

## Install

```bash
# mise
mise install ubi:radiosilence/nano-web

# cargo
cargo install nano-web

# or grab a binary from releases
```

## Usage

```bash
nano-web serve              # serve ./public on :3000
nano-web serve ./dist -p 8080
nano-web serve --spa --dev  # SPA mode + hot reload
```

## Docker

```dockerfile
FROM ghcr.io/radiosilence/nano-web:latest
COPY ./dist /public/
```

~5MB image. Multi-arch.

## Configuration

| Flag              | Default   | Description                           |
| ----------------- | --------- | ------------------------------------- |
| `--port`, `-p`    | `3000`    | Port                                  |
| `--spa`           | `false`   | Serve index.html for missing routes   |
| `--dev`, `-d`     | `false`   | Reload files on change                |
| `--config-prefix` | `VITE_`   | Env var prefix for template injection |
| `--log-level`     | `info`    | debug/info/warn/error                 |
| `--log-format`    | `console` | console/json                          |
| `--log-requests`  | `false`   | Log each request                      |

## Runtime Config Injection

Inject environment variables into HTML at startup. Useful for deploying the same build to different environments.

```html
<script>
  window.ENV = JSON.parse("{{EscapedJson}}");
</script>
```

```bash
docker run -e VITE_API_URL=https://api.prod.com my-app
```

Variables matching the prefix (default `VITE_`) get injected. Available template vars:

- `{{env.VAR_NAME}}` - single variable
- `{{Json}}` - all vars as JSON object
- `{{EscapedJson}}` - escaped for use in strings

## Health Check

`GET /_health` returns `{"status":"ok","timestamp":"..."}`.

## License

MIT
