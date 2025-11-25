# nano-web (monoio branch)

**EXPERIMENTAL** - monoio/io_uring runtime exploration. Results: slower than tokio.

## Benchmark Results

| Runtime            | Platform | Req/sec     | Avg Latency |
| ------------------ | -------- | ----------- | ----------- |
| Tokio/Hyper (main) | Linux    | **87,262**  | **0.57ms**  |
| Monoio/io_uring    | Linux    | 82,678      | 0.60ms      |
| Tokio/Hyper (main) | macOS    | **159,834** | **0.28ms**  |
| Monoio/kqueue      | macOS    | 125,355     | 0.40ms      |

**Conclusion**: Tokio wins. The overhead from monoio-compat's hyper adapter and per-worker cache duplication outweighs any io_uring syscall savings. For an in-memory static server, syscalls aren't the bottleneck.

---

Static file server. Pre-loads and pre-compresses all files at startup.

## Changes from main

- Replaces tokio with monoio thread-per-core runtime
- Uses io_uring on Linux 5.6+, kqueue on macOS
- Each worker has own route cache (no cross-thread sync)
- Hyper via monoio-compat adapter layer

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

## Health Check

`/_health` returns `{"status":"ok","timestamp":"..."}`

## License

MIT
