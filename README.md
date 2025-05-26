# 🔥 nano-web

![publish-container](https://github.com/radiosilence/nano-web/actions/workflows/publish-container.yml/badge.svg) ![push-package-amd64](https://github.com/radiosilence/nano-web/actions/workflows/push-package-amd64.yml/badge.svg) [![Go Report Card](https://goreportcard.com/badge/github.com/radiosilence/nano-web)](https://goreportcard.com/report/github.com/radiosilence/nano-web) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> ⚡ **Hyper-minimal, lightning-fast web server for SPAs and static content**

Built on [FastHTTP](https://github.com/valyala/fasthttp), nano-web is designed for maximum performance and minimal resource usage. Perfect for containerized deployments and unikernel environments.

## ✨ What makes nano-web different

- 🚄 **Ridiculously fast** - Pre-caches everything in memory with smart compression, serves 100k+ requests/second with sub-millisecond latency
- 📦 **Tiny footprint** - Tiny (<20MB) Docker image
- 🔧 **Runtime environment injection** - Safely inject environment variables into JS at runtime, perfect for easily configuring containers without rebuilding
- 🎯 **SPA-mode** - Supports modern single-page applications with fallback routing

## 🐳 Docker

```dockerfile
FROM ghcr.io/radiosilence/nano-web:latest
COPY ./dist /public/
```

Multi-stage builds work great too:

```dockerfile
FROM node:18-alpine AS build
WORKDIR /app
COPY . .
RUN npm run build

FROM ghcr.io/radiosilence/nano-web:latest
COPY --from=build /app/dist /public/
ENV SPA_MODE=true
ENV PORT=3000
```

## 🔧 Runtime Environment Injection

Instead of rebuilding your app for different environments, inject configuration at runtime:

```html
<!-- Your index.html -->
<script>
  window.ENV = {{.Json}};  // Runtime environment injection
</script>
```

```typescript
// Your React/Vue/whatever app
const config = window.ENV || {};
const apiUrl = config.API_URL || "fallback";
```

```bash
# Same build, different configs
docker run -e VITE_API_URL=http://localhost:3001 my-app    # dev
docker run -e VITE_API_URL=https://api.prod.com my-app    # prod
```

## ⚙️ Configuration

| Variable        | CLI Flag          | Default | Description                                       |
| --------------- | ----------------- | ------- | ------------------------------------------------- |
| `PORT`          | `--port`          | `80`    | Port to listen on                                 |
| `SPA_MODE`      | `--spa-mode`      | `false` | Enable SPA mode (serve index.html for 404s)       |
| `CONFIG_PREFIX` | `--config-prefix` | `VITE_` | Prefix for runtime environment variable injection |
| `LOG_LEVEL`     | `--log-level`     | `info`  | Logging level: `debug`, `info`, `warn`, `error`   |
| `LOG_FORMAT`    | `--log-format`    | `json`  | Log format: `json` or `console`                   |
| `LOG_REQUESTS`  | `--log-requests`  | `true`  | Enable/disable request logging                    |

### 📺 CLI Usage

#### Install via Go

```bash
go install github.com/radiosilence/nano-web@latest
```

#### Download Binary

```bash
# Download the latest release for your platform
wget https://github.com/radiosilence/nano-web/releases/latest/download/nano-web-linux-amd64.tar.gz
tar -xzf nano-web-linux-amd64.tar.gz
chmod +x nano-web-linux-amd64

# Or use the shorter name after installation
mv nano-web-linux-amd64 /usr/local/bin/nano-web
```

### Usage Examples

```bash
# Basic usage - serve files from ./public/ on port 80
nano-web serve

# Serve files from custom directory on port 8080
nano-web serve ./dist --port 8080

# Enable SPA mode with custom configuration
nano-web serve ./build --port 3000 --spa-mode --log-level debug

# See all available options
nano-web --help
nano-web serve --help

# Health check (useful for monitoring)
nano-web health-check

# Show version
nano-web version
```

### Build and Run

```bash
# Build the unikernel image
ops image create -c config.json --package radiosilence/nano-web:latest -i my-website

# Test locally
ops instance create my-website -c ./config.json --port 8080

# Deploy to cloud
ops instance create my-website -c ./config.json -t gcp
```

### Build and Run

```bash
# Build the unikernel image
ops image create -c config.json --package radiosilence/nano-web:latest -i my-website

# Test locally
ops instance create my-website -c ./config.json --port 8080

# Deploy to cloud
ops instance create my-website -c ./config.json -t gcp
```

**⚠️ Public config only** - don't put secrets here.

### Development Setup

```bash
# Clone and setup
git clone https://github.com/radiosilence/nano-web.git
cd nano-web

# Install development dependencies
task install-deps

# Download Go dependencies
task deps

# Run in development mode
task dev

# Or run directly with go
task run

# Run all checks (tests, lint, vet)
task check

# Build for production
task build

# Health check
task health

# Show version and build info
task info

# Create a release
task release-local

# Clean build artifacts
task clean-all
```

## 📊 Logging

Structured JSON by default, console format for development:

```bash
# Production (JSON)
{"level":"info","time":"2024-01-15T10:30:45Z","message":"request served","method":"GET","path":"/","status":200,"duration_ms":1.2}

# Development (console)
nano-web serve --log-format console --log-level debug
2024-01-15T10:30:45Z INF request served method=GET path=/ status=200 duration_ms=1.2
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

# Development server with hot reload
task dev
```

## 📈 Performance

nano-web pre-caches everything in memory with compression, which makes it fast:

```bash
wrk -d 10 -c 20 -t 10 http://localhost:80
  1,012,393 requests in 10.10s, 7.12GB read
Requests/sec: 100,237
Transfer/sec: 721MB/s
Latency: 200μs avg (96.93% consistency)
```

The trade-off is simple: use more memory at startup for faster requests. Your results will vary based on content size and hardware, but the approach is consistent.

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) for details.

### Development Setup

```bash
# Clone and setup
git clone https://github.com/radiosilence/nano-web.git
cd nano-web

# Install dependencies
task deps

# Run in development mode
task dev

# Run tests
task test
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [FastHTTP](https://github.com/valyala/fasthttp) - The blazing fast HTTP library
- [Zerolog](https://github.com/rs/zerolog) - Structured logging library
- [Brotli](https://github.com/google/brotli) - Compression algorithm

---

<div align="center">

**[Website](https://nano-web.dev)** • **[Documentation](https://docs.nano-web.dev)** • **[Examples](https://github.com/radiosilence/nano-web/tree/main/examples)**

Made with ❤️ by the nano-web team

</div>
