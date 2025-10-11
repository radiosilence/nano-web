use anyhow::{Context, Result};
#[cfg(feature = "ebpf")]
use aya::maps::HashMap as BpfHashMap;
#[cfg(feature = "ebpf")]
use aya::{
    programs::{Xdp, XdpFlags},
    Bpf,
};
use std::path::Path;
use tracing::{debug, info, warn};

use crate::response_buffer::{Encoding, ResponseBuffer};
use crate::routes::NanoWeb;

// eBPF map key structure (must match ebpf/http_server.c)
#[repr(C, packed)]
#[derive(Copy, Clone)]
struct ResponseKey {
    path_hash: u32,
    encoding: u8,
}

// eBPF map value structure (must match ebpf/http_server.c)
#[repr(C)]
struct ResponseValue {
    body_len: u32,
    content_type: [u8; 64],
    content_encoding: [u8; 16],
    etag: [u8; 64],
    cache_control: [u8; 64],
    body: [u8; 4096],
}

/// Try to start eBPF/XDP HTTP server
/// Returns handle that keeps eBPF program loaded, or error if eBPF unavailable
#[cfg(feature = "ebpf")]
pub async fn try_start_ebpf(server: &NanoWeb, interface: &str) -> Result<EbpfHandle> {
    // Check if we're root or have CAP_BPF
    if !nix::unistd::geteuid().is_root() {
        anyhow::bail!("eBPF requires root or CAP_BPF capability");
    }

    // Load compiled eBPF program
    let ebpf_path = Path::new("ebpf/http_server.o");
    if !ebpf_path.exists() {
        anyhow::bail!("eBPF program not found at {:?}", ebpf_path);
    }

    info!("Loading eBPF program from {:?}", ebpf_path);
    let mut bpf = Bpf::load_file(ebpf_path).context("Failed to load eBPF program")?;

    // Get reference to response map
    let mut response_map: BpfHashMap<_, ResponseKey, ResponseValue> = BpfHashMap::try_from(
        bpf.map_mut("response_map")
            .context("response_map not found")?,
    )
    .context("Failed to get response_map")?;

    // Populate eBPF map from our ultra_cache
    info!(
        "Populating eBPF map with {} entries",
        server.ultra_cache.len()
    );
    let mut loaded = 0;

    for entry in server.ultra_cache.iter() {
        let ((path, encoding), response_buf) = (entry.key(), entry.value());

        // Only load responses that fit in 4KB body limit
        if response_buf.body.len() > 4096 {
            debug!(
                "Skipping {} (too large: {} bytes)",
                path,
                response_buf.body.len()
            );
            continue;
        }

        let path_hash = djb2_hash(path.as_bytes());
        let encoding_byte = match encoding {
            Encoding::Identity => 0,
            Encoding::Gzip => 1,
            Encoding::Brotli => 2,
            Encoding::Zstd => 3,
        };

        let key = ResponseKey {
            path_hash,
            encoding: encoding_byte,
        };

        let mut value = ResponseValue {
            body_len: response_buf.body.len() as u32,
            content_type: [0; 64],
            content_encoding: [0; 16],
            etag: [0; 64],
            cache_control: [0; 64],
            body: [0; 4096],
        };

        // Copy strings to fixed-size arrays
        copy_str_to_array(&response_buf.content_type, &mut value.content_type);
        if let Some(enc) = response_buf.content_encoding {
            copy_str_to_array(enc, &mut value.content_encoding);
        }
        copy_str_to_array(&response_buf.etag, &mut value.etag);
        copy_str_to_array(&response_buf.cache_control, &mut value.cache_control);

        // Copy body
        let body_len = response_buf.body.len().min(4096);
        value.body[..body_len].copy_from_slice(&response_buf.body[..body_len]);

        response_map
            .insert(&key, &value, 0)
            .context("Failed to insert into eBPF map")?;

        loaded += 1;
    }

    info!(
        "Loaded {} entries into eBPF map ({} skipped)",
        loaded,
        server.ultra_cache.len() - loaded
    );

    // Attach XDP program to network interface
    let program: &mut Xdp = bpf
        .program_mut("http_server_xdp")
        .context("http_server_xdp program not found")?
        .try_into()
        .context("Program is not XDP")?;

    program.load().context("Failed to load XDP program")?;
    program
        .attach(interface, XdpFlags::SKB_MODE)
        .context(format!("Failed to attach XDP to {}", interface))?;

    info!("🚀 eBPF/XDP HTTP server attached to {}", interface);

    Ok(EbpfHandle {
        _bpf: bpf,
        interface: interface.to_string(),
    })
}

#[cfg(not(feature = "ebpf"))]
pub async fn try_start_ebpf(_server: &NanoWeb, _interface: &str) -> Result<EbpfHandle> {
    anyhow::bail!("eBPF support not compiled in (use --features ebpf)")
}

/// Handle that keeps eBPF program loaded
/// When dropped, XDP program is automatically detached
pub struct EbpfHandle {
    #[cfg(feature = "ebpf")]
    _bpf: Bpf,
    interface: String,
}

impl Drop for EbpfHandle {
    fn drop(&mut self) {
        info!("Detaching eBPF/XDP from {}", self.interface);
    }
}

// djb2 hash (must match eBPF implementation)
fn djb2_hash(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 5381;
    for &b in bytes {
        if b == 0 || b == b' ' || b == b'\r' || b == b'\n' {
            break;
        }
        hash = hash
            .wrapping_shl(5)
            .wrapping_add(hash)
            .wrapping_add(b as u32);
    }
    hash
}

fn copy_str_to_array(s: &str, arr: &mut [u8]) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(arr.len() - 1);
    arr[..len].copy_from_slice(&bytes[..len]);
    arr[len] = 0; // null terminator
}
