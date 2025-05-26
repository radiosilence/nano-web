# 🚀 nano-web

![publish-container](https://github.com/radiosilence/nano-web/actions/workflows/publish-container.yml/badge.svg) ![push-package-amd64](https://github.com/radiosilence/nano-web/actions/workflows/push-package-amd64.yml/badge.svg) [![Go Report Card](https://goreportcard.com/badge/github.com/radiosilence/nano-web)](https://goreportcard.com/report/github.com/radiosilence/nano-web) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> ⚡ **Hyper-minimal, lightning-fast web server for SPAs and static content**

Built on [FastHTTP](https://github.com/valyala/fasthttp), nano-web is designed for maximum performance and minimal resource usage. Perfect for containerized deployments, edge computing, and unikernel environments.

## ✨ Features

- 🚄 **Ultra-fast**: Built on FastHTTP for maximum performance
- 💾 **Memory-optimized**: Pre-caches and compresses all resources at startup
- 🗜️ **Smart compression**: Automatic Brotli and Gzip compression
- 🎯 **SPA-ready**: Built-in support for Single Page Applications
- 🔧 **Runtime templating**: Environment variable injection for dynamic configuration
- 📦 **Container-first**: Optimized for Docker and unikernel deployments
- 📊 **Structured logging**: JSON logging with configurable levels
- 🎨 **Zero-config**: Works out of the box with sensible defaults

## 🚀 Quick Start

### CLI Usage

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

#### Usage Examples
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
</edits>

### Docker

```dockerfile
FROM ghcr.io/radiosilence/nano-web:latest
COPY ./dist /public/
ENV PORT=8080
ENV SPA_MODE=true
ENV LOG_LEVEL=info
```
</edits>

<edits>

<old_text>
## ⚙️ Configuration

Configuration can be done via CLI flags, environment variables, or a combination of both. CLI flags take precedence over environment variables.

### CLI Flags

```bash
nano-web <command>

Commands:
  serve           Start the web server (default)
  health-check    Perform health check and exit
  version         Show version information

# For the serve command:
nano-web serve [PUBLIC_DIR] [flags]

Arguments:
  PUBLIC_DIR                  Directory containing static files to serve (default: "public")

Flags:
  -p, --port INT              Port to listen on (default: 80)
  -s, --spa-mode              Enable SPA mode (serve index.html for 404s)
      --config-prefix STRING  Prefix for runtime environment variable injection (default: "VITE_")
      --log-level STRING      Logging level (debug|info|warn|error) (default: "info")
      --log-format STRING     Log format (json|console) (default: "json")
      --log-requests          Enable request logging (default: true)
  -h, --help                  Show context-sensitive help
```

### Environment Variables

| Variable | CLI Flag | Default | Description |
|----------|----------|---------|-------------|
| `PORT` | `--port` | `80` | Port to listen on |
| `SPA_MODE` | `--spa-mode` | `false` | Enable SPA mode (serve index.html for 404s) |
| `CONFIG_PREFIX` | `--config-prefix` | `VITE_` | Prefix for runtime environment variable injection |
| `LOG_LEVEL` | `--log-level` | `info` | Logging level: `debug`, `info`, `warn`, `error` |
| `LOG_FORMAT` | `--log-format` | `json` | Log format: `json` or `console` |
| `LOG_REQUESTS` | `--log-requests` | `true` | Enable/disable request logging |

### Usage Examples

```bash
# Basic usage with defaults
nano-web serve

# Serve custom directory on different port
nano-web serve ./dist --port 8080

# SPA mode with console logging for development
nano-web serve ./build --spa-mode --log-format console --log-level debug

# Production setup with custom config prefix
nano-web serve /var/www --port 443 --config-prefix REACT_APP_ --log-level warn

# Health check (useful for Docker health checks)
nano-web health-check

# Show version
nano-web version
```

## ⚙️ Configuration

All configuration is done via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `80` | Port to listen on |
| `PUBLIC_DIR` | `public` | Directory containing static files |
| `SPA_MODE` | `0` | Set to `1` to serve `index.html` for 404s (SPA routing) |
| `CONFIG_PREFIX` | `VITE_` | Prefix for runtime environment variable injection |
| `LOG_LEVEL` | `info` | Logging level: `debug`, `info`, `warn`, `error` |
| `LOG_FORMAT` | `json` | Log format: `json` or `console` |
| `LOG_REQUESTS` | `true` | Enable/disable request logging |

## 🐳 Docker Examples

### Simple Static Site

```dockerfile
FROM ghcr.io/radiosilence/nano-web:latest
COPY ./dist /public/
ENV PORT=8080
EXPOSE 8080
```

### SPA with Runtime Configuration

```dockerfile
FROM ghcr.io/radiosilence/nano-web:latest
COPY ./build /public/
ENV PORT=8080
ENV SPA_MODE=true
ENV CONFIG_PREFIX=REACT_APP_
ENV LOG_LEVEL=warn
EXPOSE 8080
```

### Multi-stage Build

```dockerfile
# Build stage
FROM node:18-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production
COPY . .
RUN npm run build

# Runtime stage
FROM ghcr.io/radiosilence/nano-web:latest
COPY --from=builder /app/dist /public/
ENV PORT=8080
ENV SPA_MODE=true
EXPOSE 8080
```

### CLI in Docker

You can also use CLI flags directly in Docker:

```dockerfile
FROM ghcr.io/radiosilence/nano-web:latest
COPY ./dist /app/
EXPOSE 8080
CMD ["nano-web", "serve", "/app", "--port", "8080", "--spa-mode", "--log-level", "info"]
```
</edits>

<edits>

<old_text>
### Configuration

Create a `config.json`:

```json
{
  "Dirs": ["public"],
  "Args": ["serve", "./public", "--port", "8080", "--spa-mode", "--log-level", "info"],
  "RunConfig": {
    "Ports": ["8080"]
  }
}
```

Alternatively, use environment variables:

```json
{
  "Dirs": ["public"],
  "Env": {
    "PORT": "8080",
    "SPA_MODE": "true",
    "LOG_LEVEL": "info"
  },
  "RunConfig": {
    "Ports": ["8080"]
  }
}
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

## 🎯 Nanos/OPS Unikernel

Perfect for ultra-lightweight unikernel deployments:

### Configuration

Create a `config.json`:

```json
{
  "Dirs": ["public"],
  "Env": {
    "SPA_MODE": "1",
    "PORT": "8080",
    "LOG_LEVEL": "info"
  },
  "RunConfig": {
    "Ports": ["8080"]
  }
}
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

## 🔧 Runtime Configuration for SPAs

**⚠️ Important**: This feature is designed for public configuration only. Never expose secrets through this mechanism.

nano-web supports runtime environment variable injection, perfect for dynamic API endpoints, feature flags, and client-side configuration.

### HTML Template

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8">
    <title>My App</title>
    <script>
      // Inject runtime environment variables
      window.ENV = {{.Json}};
      window.ENV_ESCAPED = "{{.EscapedJson}}";
    </script>
  </head>
  <body>
    <div id="root"></div>
  </body>
</html>
```

### JavaScript/TypeScript Usage

```typescript
// Safe runtime environment access
let runtimeEnv: Record<string, string> = {};

try {
  runtimeEnv = window.ENV || JSON.parse(window.ENV_ESCAPED || "{}");
} catch (error) {
  console.warn("Failed to parse runtime environment:", error);
}

// Use configuration
const apiUrl = runtimeEnv.API_URL || "https://api.example.com";
const enableFeature = runtimeEnv.ENABLE_FEATURE === "true";
```

### Deployment Example

```bash
# Development
docker run -e VITE_API_URL=http://localhost:3001 -e VITE_DEBUG=true my-app

# Production
docker run -e VITE_API_URL=https://api.prod.com -e VITE_DEBUG=false my-app

# Using CLI flags instead of environment variables
docker run my-app nano-web serve ./dist --port 8080 --spa-mode --config-prefix REACT_APP_
```
</edits>

<edits>

<old_text>
```bash
# Clone and setup
git clone https://github.com/radiosilence/nano-web.git
cd nano-web

# Install dependencies
go mod download

# Run in development mode with CLI flags
go run main.go serve ./public --port 8080 --log-format console --log-level debug

# Or using environment variables
LOG_FORMAT=console LOG_LEVEL=debug go run main.go serve

# Run tests
go test -v ./...

# Build for production
go build -o nano-web main.go

# Health check
./nano-web health-check

# Show version
./nano-web version

# Test serve command
./nano-web serve ./public --port 8080 --log-format console
```

## 📊 Logging

nano-web provides structured JSON logging perfect for log aggregation systems like Datadog, ELK stack, or Splunk.

### JSON Format (Production)

```json
{
  "level": "info",
  "time": "2024-01-15T10:30:45Z",
  "message": "request served",
  "method": "GET",
  "path": "/api/users",
  "user_agent": "Mozilla/5.0...",
  "status": 200,
  "content_length": 1024,
  "duration_ms": 15.5
}
```

### Console Format (Development)

```
2024-01-15T10:30:45Z INF request served method=GET path=/api/users status=200 duration_ms=15.5
```

### Log Levels

- `debug`: Detailed information for debugging
- `info`: General operational messages
- `warn`: Warning messages for unusual but handled situations
- `error`: Error messages for failures

## 🏗️ Building from Source

```bash
# Clone the repository
git clone https://github.com/radiosilence/nano-web.git
cd nano-web

# Build for your platform
go build -o nano-web main.go

# Build for Linux (common for containers)
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -o nano-web-linux-amd64 main.go

# Run tests
go test ./...
```

## 📈 Performance

nano-web is engineered for extreme performance with extensive optimizations:

### 🚀 Benchmark Results

**MacBook M3 Max (36GB RAM)**
```bash
wrk -d 10 -c 20 -t 10 http://localhost:80
Running 10s test @ http://localhost:80
  10 threads and 20 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency   200.07us  130.48us   8.53ms   96.93%
    Req/Sec    10.07k   352.62    11.09k    85.05%
  1012393 requests in 10.10s, 7.12GB read
Requests/sec: 100237.75
Transfer/sec:    721.45MB
```

**Key Metrics:**
- 🎯 **100K+ requests/second** - Sub-millisecond response times
- ⚡ **200μs average latency** - Consistently fast responses
- 📊 **721MB/s throughput** - High data transfer rates
- 🎪 **96.93% latency consistency** - Predictable performance

### ⚡ Performance Optimizations

- **Zero-allocation request handling**: Pre-allocated byte slices and buffer pools
- **Memory-mapped content**: All assets pre-loaded and compressed at startup
- **Optimized FastHTTP configuration**: Custom server tuning for maximum throughput
- **Atomic operations**: Lock-free request counters and statistics
- **Byte-level operations**: Direct byte comparisons for headers and paths
- **Smart compression**: Pre-compressed Brotli and Gzip content stored in memory
- **Efficient routing**: RWMutex for concurrent reads with minimal locking

Performance characteristics will vary based on your content size, server specifications, and traffic patterns. The pre-caching approach trades startup time and memory usage for exceptional request latency.

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) for details.

### Development Setup

```bash
# Clone and setup
git clone https://github.com/radiosilence/nano-web.git
cd nano-web

# Install dependencies
go mod download

# Run in development mode
LOG_FORMAT=console LOG_LEVEL=debug go run main.go

# Run tests
go test -v ./...
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