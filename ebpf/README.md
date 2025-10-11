# eBPF XDP HTTP Server (Experimental)

**MAXIMUM PERFORMANCE MODE**: Serve HTTP responses directly from kernel space.

## Architecture

```
NIC RX → XDP Hook → Parse HTTP → BPF Map Lookup → Build Response → XDP_TX → NIC TX
         ↑                                                                      ↓
         └──────────────────── ALL IN KERNEL ──────────────────────────────────┘
```

### Flow

1. Packet arrives at NIC
2. XDP program runs **before kernel network stack**
3. Parse Ethernet → IP → TCP → HTTP headers
4. Extract path from "GET /path HTTP/1.1"
5. Hash path, lookup in eBPF map
6. If found: build HTTP response, swap src/dst, send via XDP_TX
7. If not found: pass to userspace (XDP_PASS)

### Why This Is Insane

**Performance:**

- Cloudflare: 10M packets/sec on single core with XDP
- Potential: 1M+ req/s for simple GET requests
- Latency: ~10-50μs (vs 334μs in userspace)

**Trade-offs:**

- HTTP/1.1 only, no keep-alive (stateless per-packet)
- No TLS (would need kernel TLS + eBPF integration)
- Limited to ~4KB responses (eBPF map value size)
- No dynamic content (everything pre-loaded)
- Debugging is hell
- Linux only

## Current Status

**PROOF OF CONCEPT** - Not production ready.

- [x] Basic XDP skeleton
- [x] HTTP GET parser (minimal)
- [x] eBPF map structure
- [ ] Complete packet builder
- [ ] Rust loader
- [ ] TCP checksum calculation
- [ ] Accept-Encoding parsing
- [ ] Error responses (404, etc)
- [ ] Testing

## Building

### Prerequisites (Linux only)

```bash
# Ubuntu/Debian
sudo apt-get install clang llvm linux-headers-$(uname -r)

# Fedora/RHEL
sudo dnf install clang llvm kernel-devel

# Arch
sudo pacman -S clang llvm linux-headers
```

### Compile eBPF Program

```bash
cd ebpf
make

# Or manually:
clang -O2 -g -target bpf -c http_server.c -o http_server.o
```

### Run nano-web with eBPF

```bash
# Build with eBPF feature (Linux only)
cargo build --release --features ebpf

# Run (requires root or CAP_BPF)
sudo ./target/release/nano-web serve

# Or on specific interface:
sudo ./target/release/nano-web serve --interface eth0
```

The server will automatically:

1. Try to load eBPF/XDP program
2. Populate kernel maps with pre-baked responses
3. Attach to network interface
4. Fall back to userspace if eBPF unavailable

### Manual eBPF Loading

```bash
# Attach XDP program
sudo ip link set dev lo xdp obj http_server.o

# Check if loaded
ip link show lo

# Detach
sudo ip link set dev lo xdp off
```

## Limitations

- eBPF verifier instruction limit (~1M instructions)
- Map value size limit (4KB in this implementation)
- No complex HTTP parsing (simple GET only)
- No connection tracking
- macOS/BSD not supported (Linux only)

## Future Ideas

- Multiple map lookups for large responses (chunking)
- Fallback to userspace for complex requests
- Integration with main nano-web server
- Kernel TLS support for HTTPS

## References

- [XDP Tutorial](https://github.com/xdp-project/xdp-tutorial)
- [eBPF Docs](https://ebpf.io/)
- [Cilium eBPF Library](https://github.com/cilium/ebpf)
