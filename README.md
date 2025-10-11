# nano-web

![CI](https://github.com/radiosilence/nano-web/actions/workflows/ci.yml/badge.svg)
![Build](https://github.com/radiosilence/nano-web/actions/workflows/build.yml/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/nano-web.svg)](https://crates.io/crates/nano-web)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

Static file server built with Rust. Pre-loads and pre-compresses _all_ files at startup to be served with near-zero latency or waiting for disk-caching.

## Performance

- Axum/Hyper HTTP stack
- Files pre-compressed at startup (brotli/gzip/zstd)
- Lock-free concurrent HashMap routing
- Zero-copy serving with Bytes

Benchmark (M3 Max 36GB):

```bash
wrk -c 50 -d 10 -t 50 http://localhost:3000
Running 10s test @ http://localhost:3000
 50 threads and 50 connections
 Thread Stats   Avg      Stdev     Max   +/- Stdev
   Latency   328.63us   47.98us   2.86ms   88.05%
   Req/Sec     3.01k   103.63     3.21k    91.58%
 1513328 requests in 10.10s, 8.58GB read
Requests/sec: 149838.48
Transfer/sec:    870.24MB
```

I know web-server benchmarks are mostly useless, however, this server does very little, and this is it's literal use case (serve file quick!).

### TechEmpower Benchmarks

TechEmpower benchmark setup ready in `techempower/` directory. See [techempower/README.md](techempower/README.md) for submission details.

The benchmark serves responses as actual pre-loaded files from memory (because why not?). Both plaintext and JSON tests work as expected.

## Features

- In-memory file serving with compression
- SPA mode with index.html fallback
- Dev mode with file watching
- Health endpoint at `/_health`
- Runtime environment variable injection
- JSON/console logging
- Docker image size of ~5MB

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
| `LOG_REQUESTS`  | `--log-requests`  | `false`   | Enable request logging                      |

### Environment Variables

```bash
# Docker example
docker run -p 3000:3000 -e PORT=3000 -e SPA_MODE=true ghcr.io/radiosilence/nano-web:latest
```

## ⚡ Runtime Environment Injection

Inject configuration at runtime without rebuilding, so you can re-use the same image for different things quickly and easily (or distribute it).

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
2025-08-12T18:15:00.990620Z  INFO nano_web::routes: Processing 18 files in parallel
   at src/routes.rs:70

 2025-08-12T18:15:01.207449Z  INFO nano_web::routes: Routes populated: 20 routes
   at src/routes.rs:102

 2025-08-12T18:15:01.207466Z  INFO nano_web::server: Routes loaded: 20
   at src/server.rs:46

 2025-08-12T18:15:01.207564Z  INFO nano_web::server: Starting server on 0.0.0.0:3001
   at src/server.rs:53
```

JSON format for log aggregation:

```json
{"timestamp":"2025-08-12T18:16:23.667689Z","level":"INFO","fields":{"message":"Processing 18 files in parallel"},"target":"nano_web::routes"}
{"timestamp":"2025-08-12T18:16:23.878820Z","level":"INFO","fields":{"message":"Routes populated: 20 routes"},"target":"nano_web::routes"}
{"timestamp":"2025-08-12T18:16:23.878843Z","level":"INFO","fields":{"message":"Routes loaded: 20"},"target":"nano_web::server"}
{"timestamp":"2025-08-12T18:16:23.878998Z","level":"INFO","fields":{"message":"Starting server on 0.0.0.0:3001"},"target":"nano_web::server"}
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

Compared to previous Go version: 80% faster (150k vs 76k req/sec), lower latency.

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
