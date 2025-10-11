# eBPF/XDP Implementation Handover

## Status

eBPF/XDP kernel-space HTTP server implementation is **structurally complete** but **untested**. Requires Linux environment for compilation and testing.

## What's Implemented

### Architecture

Hybrid design: eBPF handles simple GET requests in kernel space, userspace handles everything else.

- **eBPF Program** (`ebpf/http_server.c`): XDP hook that parses HTTP, looks up responses in BPF map, builds complete packets, returns XDP_TX
- **Rust Loader** (`src/ebpf_server.rs`): Loads eBPF program, populates maps from ultra_cache, attaches to interface
- **Auto-detection** (`src/server.rs`): Attempts eBPF startup on Linux, gracefully falls back to userspace on failure
- **Feature Flag** (`Cargo.toml`): `--features ebpf` adds aya dependencies (Linux only)

### eBPF Program (`ebpf/http_server.c`)

Complete XDP packet processor:

1. **Parse incoming packet**: Ethernet → IP → TCP → HTTP GET request
2. **Extract path**: Parse "GET /path HTTP/1.1"
3. **Hash path**: djb2 hash for BPF map lookup
4. **Lookup response**: BPF_MAP_TYPE_HASH with (path_hash, encoding) key
5. **Build response packet**:
   - Swap Ethernet MAC addresses
   - Swap IP addresses
   - Swap TCP ports
   - Set TCP flags (ACK)
   - Build HTTP response headers
   - Copy body from map
6. **Adjust packet size**: `bpf_xdp_adjust_tail(ctx, delta)` to grow/shrink
7. **Calculate checksums**: IP and TCP checksums
8. **Transmit**: `return XDP_TX`

**Map structure**:
```c
struct response_key {
    u32 path_hash;  // djb2 hash
    u8 encoding;    // 0=none, 1=gzip, 2=brotli, 3=zstd
} __attribute__((packed));

struct response_value {
    u32 body_len;
    u8 content_type[64];
    u8 content_encoding[16];
    u8 etag[64];
    u8 cache_control[64];
    u8 body[4096];  // Max 4KB due to kernel stack limits
};
```

### Rust Loader (`src/ebpf_server.rs`)

Loads eBPF and populates map:

1. **Load compiled eBPF**: `Bpf::load_file("ebpf/http_server.o")`
2. **Get map handle**: `HashMap::<_, ResponseKey, ResponseValue>::try_from(bpf.map_mut("response_map"))`
3. **Populate from ultra_cache**: Iterate all cached responses, hash paths, insert into map
4. **Attach XDP program**: `xdp.attach(interface, XdpFlags::default())`
5. **Return handle**: Keeps eBPF loaded until dropped

**Auto-detection** in `src/server.rs:53`:
```rust
#[cfg(target_os = "linux")]
let _ebpf_handle = {
    match crate::ebpf_server::try_start_ebpf(&server, "lo").await {
        Ok(handle) => {
            info!("🚀 eBPF/XDP: Kernel-space HTTP acceleration enabled");
            Some(handle)
        }
        Err(e) => {
            warn!("eBPF unavailable: {}", e);
            None
        }
    }
};
```

## What Needs Testing

### 1. Compile eBPF Program

```bash
cd ebpf
make
```

**Expected**: Generates `http_server.o` in `ebpf/` directory

**Potential issues**:
- Missing kernel headers
- Clang/LLVM version incompatibility
- BPF helper function availability

### 2. Build Rust with eBPF Feature

```bash
cargo build --release --features ebpf
```

**Expected**: Clean build with aya dependencies

**Potential issues**:
- aya version compatibility with kernel
- Structure alignment between C and Rust

### 3. Run with eBPF

```bash
# Requires CAP_NET_ADMIN
sudo ./target/release/nano-web serve ./public
```

**Expected log output**:
```
🚀 eBPF/XDP: Kernel-space HTTP acceleration enabled
Starting server on 0.0.0.0:3000
```

**Fallback log** (if eBPF unavailable):
```
eBPF unavailable: <error>
Starting server on 0.0.0.0:3000
```

### 4. Test Basic Request

```bash
curl -v http://localhost:3000/
```

**Expected**: Served from eBPF (should be fucking fast)

**Debug**: Check kernel logs for XDP events:
```bash
sudo cat /sys/kernel/debug/tracing/trace_pipe
```

### 5. Benchmark vs Userspace

```bash
# With eBPF
wrk -c 50 -d 10 -t 50 http://localhost:3000

# Without eBPF (run on macOS or disable feature)
wrk -c 50 -d 10 -t 50 http://localhost:3000
```

**Expected**: eBPF should hit 1M+ req/s (current userspace: 147k req/s)

### 6. Test Encoding Negotiation

```bash
curl -H "Accept-Encoding: gzip" http://localhost:3000/
curl -H "Accept-Encoding: br" http://localhost:3000/
curl -H "Accept-Encoding: zstd" http://localhost:3000/
```

**Verify**: Response has correct Content-Encoding header

### 7. Test Fallback for Large Files

If response body > 4KB, should automatically fall back to userspace.

**Create large file**:
```bash
dd if=/dev/zero of=./public/large.bin bs=1024 count=10
```

**Request**:
```bash
curl http://localhost:3000/large.bin
```

**Expected**: Served from userspace (not eBPF), still works correctly

## Known Limitations

1. **4KB response limit**: eBPF kernel stack constraint. Larger responses fall back to userspace.
2. **Simple GET only**: No POST, range requests, or complex headers in eBPF path.
3. **No TLS**: eBPF operates at L2/L3, before TLS termination.
4. **Linux only**: XDP is Linux kernel feature.
5. **Accept-Encoding parsing**: Currently assumes identity encoding in eBPF. Full parsing needs implementation.

## Potential Issues

### Structure Padding

C and Rust structures must match exactly. Current `ResponseKey` uses `packed` attribute:

```c
struct response_key {
    u32 path_hash;
    u8 encoding;
} __attribute__((packed));
```

```rust
#[repr(C, packed)]
struct ResponseKey {
    path_hash: u32,
    encoding: u8,
}
```

If map lookups fail, check structure sizes:
```bash
# In C
sizeof(struct response_key)

# In Rust
std::mem::size_of::<ResponseKey>()
```

### Path Hashing

djb2 implementation must match between Rust and C:

```rust
fn djb2_hash(path: &str) -> u32 {
    path.bytes().fold(5381u32, |hash, byte| {
        hash.wrapping_mul(33).wrapping_add(byte as u32)
    })
}
```

```c
static __always_inline u32 djb2_hash(const char *str, int len) {
    u32 hash = 5381;
    for (int i = 0; i < len && i < 256; i++) {
        hash = ((hash << 5) + hash) + str[i];
    }
    return hash;
}
```

### Checksum Calculation

IP and TCP checksums calculated in eBPF. If responses are malformed, verify checksums against known-good implementation.

### XDP Flags

Currently uses `XdpFlags::default()`. May need `XdpFlags::SKB_MODE` for testing on loopback or virtual interfaces.

## Debugging

### Enable eBPF Debug Logging

eBPF uses `bpf_trace_printk` (limited):

```c
bpf_trace_printk("eBPF: parsed path, length=%d\n", path_len);
```

View logs:
```bash
sudo cat /sys/kernel/debug/tracing/trace_pipe
```

### bpftool

Inspect loaded programs and maps:

```bash
# List XDP programs
sudo bpftool prog show type xdp

# Dump map contents
sudo bpftool map dump name response_map

# Show program details
sudo bpftool prog show id <ID>
```

### Packet Capture

Compare eBPF vs userspace packets:

```bash
# Capture on loopback
sudo tcpdump -i lo -w ebpf.pcap port 3000

# Analyze in Wireshark
wireshark ebpf.pcap
```

### Force Userspace

To test without eBPF:
```bash
# Build without feature
cargo build --release

# Or run on macOS/non-Linux
```

## Next Steps

1. **Compile and test** on Linux VM
2. **Benchmark** eBPF vs userspace
3. **Implement Accept-Encoding parsing** in eBPF (currently defaults to identity)
4. **Add bpf_trace_printk** debug logs for path lookup hits/misses
5. **Test different XDP modes** (native vs SKB)
6. **Profile** with perf to verify kernel-space execution

## Files Changed

- `ebpf/http_server.c` - XDP program
- `ebpf/Makefile` - Build eBPF program
- `src/ebpf_server.rs` - Rust loader
- `src/server.rs:53` - Auto-detection logic
- `Cargo.toml` - eBPF feature flag
- `README.md` - Documentation

## Reference

- XDP performance: 10M+ pps possible
- Current userspace: 147k req/s @ 334μs latency
- Target eBPF: 1M+ req/s @ <100μs latency (estimate)

Hybrid architecture means complex requests still work via Axum fallback. eBPF just handles the hot path (static GET requests).

---

**TL;DR**: Code's done, needs Linux box to actually compile and test. Auto-detects, falls back gracefully, should be stupid fast if it works.
