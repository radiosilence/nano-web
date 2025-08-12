# nano-web

![CI](https://github.com/radiosilence/nano-web/actions/workflows/ci.yml/badge.svg)
![Build](https://github.com/radiosilence/nano-web/actions/workflows/build.yml/badge.svg)
![Release](https://github.com/radiosilence/nano-web/actions/workflows/release.yml/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/nano-web.svg)](https://crates.io/crates/nano-web)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

Static file server built with Rust. Serves files from memory with pre-compressed variants.

## Performance

130,000+ requests/sec with sub-millisecond latency:

- Axum/Hyper HTTP stack
- Files pre-compressed at startup (brotli/gzip/zstd)
- Lock-free concurrent HashMap routing
- Zero-copy serving with Bytes

Benchmark (M3 Max):

```bash
wrk -c 100 -d 10 -t 100 http://localhost:3000
Running 10s test @ http://localhost:3000
  100 threads and 100 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency   766.28us  125.68us   4.42ms   92.26%
    Req/Sec     1.31k    69.24     1.54k    93.77%
  1317792 requests in 10.10s, 7.47GB read
Requests/sec: 130,479.93
Transfer/sec:    757.44MB
```

## Features

- In-memory file serving with compression
- Security headers
- SPA mode with index.html fallback
- Dev mode with file watching
- Health endpoint at `/_health`
- Runtime environment variable injection
- JSON/console logging
- Docker images under 20MB

## 📦 Installation

### Install with Mise (via ubi)

```bash
mise install ubi:radiosilence/nano-web
```

### Install with Cargo

```bash
cargo install nano-web
```

### Download Binary

Pre-built binaries available on [GitHub Releases](https://github.com/radiosilence/nano-web/releases).

## 🐳 Docker

Multi-arch images available:

```dockerfile
FROM ghcr.io/radiosilence/nano-web:latest
COPY ./dist /public/
```

Production example:

```dockerfile
FROM node:lts-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production
COPY . .
RUN npm run build

FROM ghcr.io/radiosilence/nano-web:latest
COPY --from=builder /app/dist/ /public/
EXPOSE 3000
```

## 🔧 Usage

```bash
# Serve files from ./public/ on port 3000
nano-web serve

# Custom directory and port
nano-web serve ./dist --port 8080

# SPA mode with dev reloading
nano-web serve --spa --dev --port 3000

# See all options
nano-web serve --help
```

## ⚙️ Configuration

| Variable        | CLI Flag          | Default   | Description                                 |
| --------------- | ----------------- | --------- | ------------------------------------------- |
| `PORT`          | `--port`, `-p`    | `3000`    | Port to listen on                           |
| `--spa`         | `--spa`           | `false`   | Enable SPA mode (serve index.html for 404s) |
| `--dev`         | `--dev`, `-d`     | `false`   | Enable dev mode (hot-reload files)          |
| `CONFIG_PREFIX` | `--config-prefix` | `VITE_`   | Environment variable injection prefix       |
| `LOG_LEVEL`     | `--log-level`     | `info`    | Logging: `debug`, `info`, `warn`, `error`   |
| `LOG_FORMAT`    | `--log-format`    | `console` | Format: `json` or `console`                 |
| `LOG_REQUESTS`  | `--log-requests`  | `true`    | Enable request logging                      |

### Environment Variables

```bash
# Docker example
docker run -p 3000:3000 -e PORT=3000 -e SPA_MODE=true ghcr.io/radiosilence/nano-web:latest
```

## ⚡ Runtime Environment Injection

Inject configuration at runtime without rebuilding:

```html
<!-- Your index.html -->
<script type="module">
  window.ENV = JSON.parse("{{EscapedJson}}");
  // or direct injection if you don't mind the broken syntax:
  window.ENV = {{Json}};
</script>
```

```typescript
// env.ts
import * as z from "zod";
const EnvSchema = z.object({
  API_URL: z.url(),
});
// Parse and validate environment variables
const result = EnvSchema.safeParse(window.ENV);

if (!result.success) {
  throw new Error(
    `Environment validation failed:\n${z.prettifyError(result.error)}`,
  );
}
export const { API_URL } = result.data;
```

```bash
# Same build, different configs
docker run -e VITE_API_URL=http://localhost:3001 my-app    # dev
docker run -e VITE_API_URL=https://api.prod.com my-app     # prod
```

### Template Engine

Uses MiniJinja template syntax for environment variable injection. Variables available:

- `{{env.VARIABLE_NAME}}` - Direct variable access
- `{{Json}}` - Raw JSON string of all prefixed variables
- `{{EscapedJson}}` - JSON-escaped for inline JavaScript

Templating is run at startup and cached.

## 🏥 Health Checks

Built-in health endpoint at `/_health`:

```json
{ "status": "ok", "timestamp": "2025-01-15T10:30:45Z" }
```

## 📊 Logging

Console format (default):

```
2025-01-15T10:30:45Z  INFO nano_web: Starting server on 0.0.0.0:3000
2025-01-15T10:30:45Z  INFO nano_web: Routes loaded: 15
```

JSON format for log aggregation:

```json
{
  "timestamp": "2025-01-15T10:30:45Z",
  "level": "INFO",
  "message": "request served",
  "method": "GET",
  "path": "/",
  "status": 200,
  "duration_ms": 0.766
}
```

## 🛠️ Building from Source

```bash
# Clone and build
git clone https://github.com/radiosilence/nano-web.git
cd nano-web
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench
```

### Development

```bash
# Development server with hot-reload
cargo run -- serve ./public --dev --spa

# Watch for changes
cargo watch -x "run -- serve ./public --dev"
```

## 🌰 Advanced: Unikernels

Deploy as unikernels with [Nanos](https://nanos.org):

```json
{
  "Dirs": ["public"],
  "Env": {
    "SPA_MODE": "1",
    "PORT": "8080"
  },
  "RunConfig": {
    "Ports": ["8080"]
  }
}
```

```bash
ops image create -c config.json --package nano-web:latest -i my-website
ops instance create my-website --port 8080
```

## Architecture

- HTTP: Axum + Hyper
- Routing: Lock-free DashMap with FxHash
- Compression: Parallel pre-compression at startup
- Memory: Zero-copy serving with Bytes
- Security: Path validation, security headers
- Runtime: Tokio async

Compared to previous Go version: 70% faster (130k vs 76k req/sec), lower latency, no GC overhead.

## 📄 License

Licensed under the MIT License - see [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- [Axum](https://github.com/tokio-rs/axum) - Ergonomic async web framework
- [Hyper](https://github.com/hyperium/hyper) - Fast HTTP implementation
- [Tokio](https://github.com/tokio-rs/tokio) - Asynchronous runtime
- [DashMap](https://github.com/xacrimon/dashmap) - Lock-free concurrent HashMap
- [Brotli](https://github.com/dropbox/rust-brotli) - Compression library

---

<div align="center">
    Made with 🦀 by <a href="https://github.com/radiosilence">@radiosilence</a>
</div>
