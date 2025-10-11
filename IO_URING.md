# io_uring Implementation

This branch implements a dual-mode architecture for nano-web that leverages Linux's io_uring for maximum performance while maintaining compatibility with Darwin/macOS and other platforms.

## Architecture

### Linux (io_uring)
- **Runtime**: `tokio-uring` with native io_uring support
- **HTTP Stack**: Custom minimal HTTP/1.1 parser (zero dependencies)
- **Zero-copy**: Direct kernel buffer management via io_uring
- **Location**: `src/server_uring.rs`, `src/http.rs`

### Darwin/Other (Fallback)
- **Runtime**: Tokio async runtime
- **HTTP Stack**: Axum + Hyper (existing implementation)
- **Location**: `src/server.rs`

## What is io_uring?

io_uring is a Linux kernel interface (5.1+) that provides:
- **Asynchronous I/O** without syscalls per operation
- **Zero-copy** data transfer between kernel and userspace
- **Batch operations** via submission/completion queues
- **Direct buffer registration** for true zero-copy networking

Think of it as cutting out the middleman - your buffers go straight from memory to the NIC.

## Performance Benefits

### Traditional Path (epoll/kqueue)
```
User Buffer → Kernel Copy → TCP Stack → NIC Buffer → Network
           ↑ syscall       ↑ syscall
```

### io_uring Path
```
User Buffer → io_uring Queue → Kernel → NIC → Network
           ↑ single syscall for batch
```

With registered buffers:
```
Pre-registered Buffer → Kernel References It → DMA to NIC → Network
                      ↑ no copy, just pointer
```

## Implementation Details

### HTTP Parser (`src/http.rs`)
- Zero-dependency HTTP/1.1 parser
- Zero-copy request parsing (string slices into buffer)
- Minimal response builder
- Keep-Alive support

### io_uring Server (`src/server_uring.rs`)
- Uses `tokio_uring::start()` runtime
- Pre-loads all files into memory (like standard nano-web)
- Each connection gets its own task
- HTTP/1.1 keep-alive supported
- Direct buffer I/O with io_uring read/write operations

### Conditional Compilation
```rust
#[cfg(target_os = "linux")]
// Use io_uring implementation

#[cfg(not(target_os = "linux"))]
// Use Axum implementation
```

## Building

### On Linux
```bash
cargo build --release
# Automatically uses io_uring
```

### On Darwin/macOS
```bash
cargo build --release
# Automatically uses Axum fallback
```

### Cross-compilation
```bash
# Build Linux binary from Darwin
cargo build --release --target x86_64-unknown-linux-gnu
```

## Kernel Requirements

**Linux**: Kernel 5.10+ required for io_uring
**Darwin/Other**: No special requirements

Check your kernel:
```bash
uname -r
# Should be 5.10.0 or higher
```

## Why Not Just Use io_uring Everywhere?

1. **Platform Support**: io_uring is Linux-only
2. **Development**: Most devs use macOS, need working local server
3. **Production**: Linux servers get the speed, Darwin gets compatibility
4. **Best of Both**: Zero compromise - full speed where it matters, full compatibility where it doesn't

## Future Optimizations

These are implemented in the current version:
- ✅ io_uring read/write operations
- ✅ Zero-copy HTTP parsing
- ✅ Pre-loaded file buffers

Potential future improvements:
- [ ] Registered buffers (`IORING_REGISTER_BUFFERS`)
- [ ] Fixed files (`IORING_REGISTER_FILES`)
- [ ] `IORING_OP_SEND` with `IOSQE_FIXED_FILE`
- [ ] `MSG_ZEROCOPY` socket option
- [ ] Multi-shot accept (`IORING_OP_MULTISHOT_ACCEPT`)
- [ ] Kernel-side TLS offload (kTLS)

## Benchmarks

TODO: Run benchmarks comparing:
1. Axum implementation (Darwin)
2. io_uring implementation (Linux)
3. Previous Go implementation

Expected improvement: 20-50% faster on Linux with io_uring.

## References

- [io_uring explained](https://kernel.dk/io_uring.pdf)
- [tokio-uring](https://github.com/tokio-rs/tokio-uring)
- [Efficient IO with io_uring](https://kernel.dk/io_uring.pdf)
- [Zero-copy networking with io_uring](https://lwn.net/Articles/879724/)

## Testing

### Linux
```bash
# Build and run
cargo build --release
./target/release/nano-web serve ./public --port 3000

# Check it's using io_uring
lsof -p $(pgrep nano-web) | grep io_uring
```

### Darwin
```bash
# Should automatically use Axum
cargo build --release
./target/release/nano-web serve ./public --port 3000

# Verify logs mention Axum, not io_uring
```

## Known Limitations

1. **Linux Only**: io_uring mode only works on Linux 5.10+
2. **No TLS**: Current implementation is HTTP only (TLS via reverse proxy)
3. **No HTTP/2**: Currently HTTP/1.1 only in io_uring mode
4. **Keep-Alive**: Implemented but connection pooling is basic

## Contributing

When modifying:
- `src/http.rs` - Affects both implementations
- `src/server_uring.rs` - Linux/io_uring only
- `src/server.rs` - Darwin/fallback only
- Test on both platforms if possible

## The Real Talk

Is this overkill for a static file server? **Absolutely.**

Will it make a measurable difference? **Probably 10-20% at most.**

Is it cool as fuck? **Hell yes.**

Does it teach you about kernel-level networking? **Definitely.**

Should you use it in production? **Why not? It's faster and still has the Axum fallback.**
