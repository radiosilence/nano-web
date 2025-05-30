# 🔥 nano-web

![publish-image](https://github.com/radiosilence/nano-web/actions/workflows/publish-image.yml/badge.svg)
![push-package-amd64](https://github.com/radiosilence/nano-web/actions/workflows/push-package-amd64.yml/badge.svg)
![release](https://github.com/radiosilence/nano-web/actions/workflows/release.yml/badge.svg)
![test](https://github.com/radiosilence/nano-web/actions/workflows/test.yml/badge.svg)
[![Go Report Card](https://goreportcard.com/badge/github.com/radiosilence/nano-web)](https://goreportcard.com/report/github.com/radiosilence/nano-web) [![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

> ⚡ **Hyper-minimal, lightning-fast web server for SPAs and static content**

Built on [FastHTTP](https://github.com/valyala/fasthttp), nano-web is designed for minimal latency. Purpose built for use with containerized deployments/unikernel environments with immutable content, however totally useable as a local CLI server.

## ✨ What makes nano-web different

- 🚄 **Ridiculously low latency** - Pre-caches everything in memory precompressed with brotli/gzip where appropriate, serves 100k+ requests/second with sub-millisecond latency.
- 📦 **Tiny footprint** - Tiny (<20MB) Docker image.
- 🔧 **Runtime environment injection** - Safely inject environment variables at runtime, so you can configure containers without rebuilding if you don't want to do different builds for things, or want to test your prod image against a different environment.
- 🚑 **Inbuilt Healthchecks** - Available at `/_health`.
- 🎯 **SPA-mode** - Supports modern single-page applications with fallback routing.
- ⚡️ **Fast builds** - Building an image from nano-web is extremely fast because it is tiny.

## 📈 Performance

nano-web pre-caches everything in memory with compression, which makes it fast. Sure you could rely on filesystem caching to do this, but knowing the content ahead of time allows us just to compress everything and stick it in RAM. Benchmark on a M3 Max 36GB:

```bash
wrk -d 10 -c 20 -t 10 http://localhost
  1,012,393 requests in 10.10s, 7.12GB read
Requests/sec: 100,237
Transfer/sec: 721MB/s
Latency: 200μs avg (96.93% consistency)
```

The trade off is basically to use more memory at startup to do less work on each request due to having predictable content. Generally it shouldn't use that much more RAM than the project by much.

## 🐳 Docker

```dockerfile
FROM ghcr.io/radiosilence/nano-web:latest
COPY ./dist /public/
```

Real-world example for running as unprivileged user (/bin/sh etc are not available for the runner):

```dockerfile
FROM oven/bun:1 AS base
RUN adduser --disabled-password --shell /bin/sh nano
WORKDIR /app

FROM base AS deps
COPY package.json ./
RUN bun install

FROM base AS builder
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN bun run build
RUN chown -R nano:nano /app/.output/public/

FROM ghcr.io/radiosilence/nano-web:latest AS runner
COPY --from=base /etc/passwd /etc/passwd
COPY --from=base /etc/group /etc/group
COPY --from=builder /app/.output/public/ /public/
USER nano
ENV PORT=3000
EXPOSE 3000
```

_This is a TanStack Start project being built statically with bun._

Configure with env vars you can see below👇

## 🔧 Runtime Environment Injection

Instead of rebuilding your app for different environments, inject configuration at runtime:

**⚠️ Public config only** - don't put secrets here.

```html
<!-- Your index.html -->
<script type="module">
  // You can wrap the escaped JSON in a string and parse it.
  window.ENV = JSON.parse("{{.EscapedJson}}");
  // You can pass in the raw JSON, but this means having the template tag directly inserted without
  // being quoted.
  window.ENV = {{.Json}};
</script>
```

```typescript
import { z } from "zod";
// Your React/Vue/whatever app
const EnvSchema = z.object({
  API_URL: z.string().optional(),
});
const { API_URL } = EnvSchema.parse(window.ENV));
```

```bash
# Same build, different configs
docker run -e VITE_API_URL=http://localhost:3001 my-app    # dev
docker run -e VITE_API_URL=https://api.prod.com my-app    # prod
```

## ⚙️ Configuration

| Variable        | CLI Flag          | Default   | Description                                                 |
| --------------- | ----------------- | --------- | ----------------------------------------------------------- |
| `PORT`          | `--port`          | `80`      | Port to listen on                                           |
| `SPA_MODE`      | `--spa-mode`      | `false`   | Enable SPA mode (serve index.html for 404s)                 |
| `DEV`           | `--dev`           | `false`   | Enable Dev mode (check for file changes when serving files) |
| `CONFIG_PREFIX` | `--config-prefix` | `VITE_`   | Prefix for runtime environment variable injection           |
| `LOG_LEVEL`     | `--log-level`     | `info`    | Logging level: `debug`, `info`, `warn`, `error`             |
| `LOG_FORMAT`    | `--log-format`    | `console` | Log format: `json` or `console`                             |
| `LOG_REQUESTS`  | `--log-requests`  | `true`    | Enable/disable request logging                              |

## 🚑 Health checks

Enabled by default at `/_health`:

```
{"status":"ok","timestamp":"2025-05-27T08:19:32Z"}
```

### 📺 CLI Usage

#### Install via Go

```bash
go install github.com/radiosilence/nano-web@latest
```

#### Usage Examples

```bash
# Basic usage - serve files from ./public/ on port 80
nano-web serve

# Serve files from custom directory on port 8080
nano-web serve ./dist --port 8080

# Enable SPA mode with custom configuration, file reloading, and debug logging (similar to `task dev`)
nano-web serve ./build --port 3000 --spa-mode --dev --log-level debug

# See all available options
nano-web --help
nano-web serve --help

# Show version
nano-web version
```

## 🌰 Nanos/OPS Microkernels

You will want to make a config that looks something like this:

```
{
  "Dirs": ["public"],
  "Env": {
    "SPA_MODE": "1",
    "PORT": "8081"
  },
  "RunConfig": {
    "Ports": ["8081"]
  }
}
```

And then you can build your unikernel image:

```bash
# Build the unikernel image
ops image create -c config.json --package radiosilence/nano-web:latest -i my-website

# Test locally
ops instance create my-website -c ./config.json --port 8080

# Deploy to cloud
ops instance create my-website -c ./config.json -t gcp
```

## 📊 Logging

Defaults to readable, colourful style (`--log-format console`):

```
9:15AM INF routes populated successfully route_count=3
9:16AM INF request handled bytes=21 duration=0.044208 method=GET path=/ status=200
```

Structured JSON for consumption by logging platforms such as DataDog etc (`--log-format json`) **(enabled by default in docker)**:

```json
{
  "level": "info",
  "time": "2024-01-15T10:30:45Z",
  "message": "request served",
  "method": "GET",
  "path": "/",
  "status": 200,
  "duration_ms": 1.2
}
```

## 🏗️ Building from Source

### Prerequisites

Install [Task](https://taskfile.dev/) for build automation:

```bash
# macOS
brew install go-task/tap/go-task

# Linux/Windows - see https://taskfile.dev/installation/
```

### Building

```bash
# Clone the repository
git clone https://github.com/radiosilence/nano-web.git
cd nano-web

# See all available tasks
task

# Build for current platform
task build

# Build for all platforms
task build-all

# Run tests
task test

# Run tests with coverage
task test-coverage

# Run benchmarks
task bench

# Development server with reloading and debug logging
task dev
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [FastHTTP](https://github.com/valyala/fasthttp) - Fast HTTP library
- [Zerolog](https://github.com/rs/zerolog) - Structured logging library
- [Brotli](https://github.com/google/brotli) - Compression algorithm

---

<div align="center">
    Made with 🖤 by <a href="https://github.com/radiosilence">@radiosilence</a>
</div>
