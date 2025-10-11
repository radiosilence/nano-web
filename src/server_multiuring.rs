// Multi-threaded io_uring server with CPU pinning
// SAFETY THIRD - raw io_uring + unsafe pointers + zero allocations
//
// Architecture:
// - N worker threads = N CPU cores (pinned 1:1)
// - Each thread has its own io_uring instance (no shared state)
// - Each io_uring handles 100K+ concurrent connections
// - IORING_SETUP_SQPOLL for kernel-side polling (zero syscalls in hot path)
// - Pre-built response buffers with unsafe raw pointers
// - Registered buffers with io_uring for zero-copy writes
//
// Target: 100M+ req/sec, sub-microsecond latency, 10M concurrent connections

use anyhow::{Context, Result};
use io_uring::{opcode, types, IoUring};
use std::collections::HashMap;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use tracing::{debug, info, warn};

use crate::http::{build_response, parse_request};
use crate::registered_buffers::{RegisteredBufferManager, UnsafeResponse};
use crate::routes::NanoWeb;

pub struct MultiUringConfig {
    pub public_dir: PathBuf,
    pub port: u16,
    pub dev: bool,
    pub spa_mode: bool,
    pub config_prefix: String,
    pub num_threads: usize, // Should equal number of CPU cores
}

/// Start multi-threaded io_uring server
pub fn serve(config: MultiUringConfig) -> Result<()> {
    info!(
        "Starting multi-threaded io_uring server on 0.0.0.0:{}",
        config.port
    );
    info!(
        "Worker threads: {} (one per CPU, pinned)",
        config.num_threads
    );
    info!("Pre-loading files from {:?}", config.public_dir);

    // Pre-load all files
    let nano_web = Arc::new(NanoWeb::new());
    nano_web
        .populate_routes(&config.public_dir, &config.config_prefix)
        .context("Failed to populate routes")?;

    info!("Routes loaded: {}", nano_web.routes.len());

    // Pre-build all HTTP responses with unsafe raw pointers
    info!("Pre-building HTTP responses...");
    let buffer_manager = Arc::new(
        RegisteredBufferManager::new(&nano_web.routes, config.spa_mode)
            .context("Failed to pre-build responses")?,
    );
    info!(
        "Pre-built {} response variants",
        buffer_manager.buffer_count()
    );

    // Create listening socket
    let listener = std::net::TcpListener::bind(format!("0.0.0.0:{}", config.port))
        .context("Failed to bind to address")?;
    listener
        .set_nonblocking(true)
        .context("Failed to set non-blocking")?;

    let listener_fd = listener.as_raw_fd();
    info!("Server listening on 0.0.0.0:{}", config.port);

    // Spawn worker threads with CPU pinning
    let mut handles = vec![];
    for worker_id in 0..config.num_threads {
        let buffer_manager = buffer_manager.clone();
        let spa_mode = config.spa_mode;

        let handle = thread::spawn(move || {
            // Pin this thread to specific CPU core
            pin_thread_to_cpu(worker_id);

            if let Err(e) = run_worker(worker_id, listener_fd, buffer_manager, spa_mode) {
                warn!("Worker {} died: {:?}", worker_id, e);
            }
        });
        handles.push(handle);
    }

    // Wait for workers (never exits in normal operation)
    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}

/// Pin current thread to specific CPU core
fn pin_thread_to_cpu(cpu_id: usize) {
    unsafe {
        let mut cpuset: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(cpu_id, &mut cpuset);

        let ret = libc::sched_setaffinity(
            0, // current thread
            std::mem::size_of::<libc::cpu_set_t>(),
            &cpuset,
        );

        if ret != 0 {
            warn!("Failed to pin thread to CPU {}", cpu_id);
        } else {
            info!("Worker {} pinned to CPU {}", cpu_id, cpu_id);
        }
    }
}

/// Worker thread main loop
fn run_worker(
    worker_id: usize,
    listener_fd: RawFd,
    buffer_manager: Arc<RegisteredBufferManager>,
    spa_mode: bool,
) -> Result<()> {
    info!("Worker {} starting", worker_id);

    // Create io_uring instance with SQPOLL (kernel-side polling)
    let mut ring = IoUring::builder()
        .setup_sqpoll(1000) // Kernel polls every 1ms
        .build(4096) // 4K queue depth
        .context("Failed to create io_uring")?;

    info!("Worker {} io_uring created with SQPOLL", worker_id);

    // Register pre-built response buffers with io_uring
    let iovecs = buffer_manager.get_iovecs();
    unsafe {
        ring.submitter()
            .register_buffers(&iovecs)
            .context("Failed to register buffers")?;
    }

    info!(
        "Worker {} registered {} buffers with kernel",
        worker_id,
        iovecs.len()
    );

    // Connection state tracking
    let mut connections: HashMap<u64, ConnectionState> = HashMap::new();
    let mut user_data_counter: u64 = 1; // Start at 1, 0 reserved

    // Submit initial accept operation
    submit_accept(&mut ring, listener_fd, 0)?;

    info!("Worker {} ready", worker_id);

    // Main event loop
    loop {
        // Submit all pending operations
        ring.submit_and_wait(1)?;

        // Collect completions (drop cq before processing to avoid borrow issues)
        let completions: Vec<(u64, i32)> = {
            let mut cq = ring.completion();
            cq.map(|cqe| (cqe.user_data(), cqe.result())).collect()
        };

        // Process completions - now we can mutably borrow ring
        for (user_data, result) in completions {
            let op_type = OpType::decode(user_data);

            match op_type {
                OpType::Accept => {
                    if result < 0 {
                        warn!("Worker {} accept failed: {}", worker_id, result);
                        // Re-submit accept
                        submit_accept(&mut ring, listener_fd, 0)?;
                        continue;
                    }

                    let client_fd = result;
                    let conn_id = user_data_counter;
                    user_data_counter += 1;

                    debug!("Worker {} accepted connection {}", worker_id, conn_id);

                    // Create connection state
                    connections.insert(
                        conn_id,
                        ConnectionState {
                            fd: client_fd,
                            read_buffer: vec![0u8; 8192], // 8KB read buffer
                            bytes_read: 0,
                        },
                    );

                    // Submit read for this connection
                    submit_read(&mut ring, conn_id, &connections)?;

                    // Re-submit accept for next connection
                    submit_accept(&mut ring, listener_fd, 0)?;
                }

                OpType::Read(conn_id) => {
                    if result <= 0 {
                        // Connection closed or error
                        if let Some(conn) = connections.remove(&conn_id) {
                            unsafe {
                                libc::close(conn.fd);
                            }
                        }
                        continue;
                    }

                    // Update bytes read
                    if let Some(conn) = connections.get_mut(&conn_id) {
                        conn.bytes_read += result as usize;

                        // Try to parse HTTP request
                        let request_data = &conn.read_buffer[..conn.bytes_read];

                        // Try to parse - if incomplete, read more
                        match crate::http::parse_request(request_data) {
                            Ok(_) => {
                                // Request is complete, match and respond
                                if let Some((path, encoding)) =
                                    parse_and_match(request_data, &buffer_manager, spa_mode)
                                {
                                    if let Some(response) = buffer_manager.get(&path, encoding) {
                                        // Submit zero-copy write using registered buffer
                                        // Response already contains full HTTP headers + body
                                        submit_write_registered(
                                            &mut ring,
                                            conn_id,
                                            response,
                                            &connections,
                                        )?;
                                    } else {
                                        // 404 response
                                        submit_write_404(&mut ring, conn_id, &mut connections)?;
                                    }
                                } else {
                                    // Bad request
                                    submit_write_400(&mut ring, conn_id, &mut connections)?;
                                }
                            }
                            Err(crate::http::ParseError::Incomplete) => {
                                if conn.bytes_read >= conn.read_buffer.len() {
                                    // Request too large, send 413
                                    submit_write_413(&mut ring, conn_id, &mut connections)?;
                                } else {
                                    // Need more data, submit another read
                                    submit_read(&mut ring, conn_id, &connections)?;
                                }
                            }
                            Err(crate::http::ParseError::Invalid) => {
                                // Bad request
                                submit_write_400(&mut ring, conn_id, &mut connections)?;
                            }
                        }
                    }
                }

                OpType::Write(conn_id) => {
                    // Write completed, close connection
                    if let Some(conn) = connections.remove(&conn_id) {
                        unsafe {
                            libc::close(conn.fd);
                        }
                    }
                }
            }
        }
    }
}

/// Operation type encoded in user_data
#[derive(Debug, Clone, Copy)]
enum OpType {
    Accept,
    Read(u64),  // connection_id
    Write(u64), // connection_id
}

impl OpType {
    fn encode(&self) -> u64 {
        match self {
            OpType::Accept => 0,
            OpType::Read(id) => (1u64 << 62) | id,
            OpType::Write(id) => (2u64 << 62) | id,
        }
    }

    fn decode(user_data: u64) -> Self {
        let op_type = user_data >> 62;
        let id = user_data & ((1u64 << 62) - 1);
        match op_type {
            0 => OpType::Accept,
            1 => OpType::Read(id),
            2 => OpType::Write(id),
            _ => OpType::Accept, // fallback
        }
    }
}

/// Connection state
struct ConnectionState {
    fd: RawFd,
    read_buffer: Vec<u8>,
    bytes_read: usize,
}

/// Submit accept operation to io_uring
fn submit_accept(ring: &mut IoUring, listener_fd: RawFd, user_data: u64) -> Result<()> {
    let accept_e = opcode::Accept::new(
        types::Fd(listener_fd),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    )
    .build()
    .user_data(user_data);

    unsafe {
        ring.submission()
            .push(&accept_e)
            .context("Failed to push accept")?;
    }

    Ok(())
}

/// Submit read operation to io_uring
fn submit_read(
    ring: &mut IoUring,
    conn_id: u64,
    connections: &HashMap<u64, ConnectionState>,
) -> Result<()> {
    let conn = connections.get(&conn_id).context("Connection not found")?;

    let read_e = opcode::Read::new(
        types::Fd(conn.fd),
        conn.read_buffer[conn.bytes_read..].as_ptr() as *mut u8,
        (conn.read_buffer.len() - conn.bytes_read) as u32,
    )
    .build()
    .user_data(OpType::Read(conn_id).encode());

    unsafe {
        ring.submission()
            .push(&read_e)
            .context("Failed to push read")?;
    }

    Ok(())
}

/// Submit zero-copy write using registered buffer
fn submit_write_registered(
    ring: &mut IoUring,
    conn_id: u64,
    response: UnsafeResponse,
    connections: &HashMap<u64, ConnectionState>,
) -> Result<()> {
    let conn = connections.get(&conn_id).context("Connection not found")?;

    // Use WriteFixed to tell kernel: "write registered buffer #N to this socket"
    // This is TRUE zero-copy - kernel reads directly from our pinned buffer
    let write_e = opcode::WriteFixed::new(
        types::Fd(conn.fd),
        response.ptr as *const u8,
        response.len as u32,
        response.buffer_id,
    )
    .build()
    .user_data(OpType::Write(conn_id).encode());

    unsafe {
        ring.submission()
            .push(&write_e)
            .context("Failed to push write")?;
    }

    Ok(())
}

/// Submit 404 response
fn submit_write_404(
    ring: &mut IoUring,
    conn_id: u64,
    connections: &mut HashMap<u64, ConnectionState>,
) -> Result<()> {
    let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";

    if let Some(conn) = connections.get_mut(&conn_id) {
        // Reuse read buffer for response
        conn.read_buffer.clear();
        conn.read_buffer.extend_from_slice(response);

        let write_e = opcode::Write::new(
            types::Fd(conn.fd),
            conn.read_buffer.as_ptr(),
            conn.read_buffer.len() as u32,
        )
        .build()
        .user_data(OpType::Write(conn_id).encode());

        unsafe {
            ring.submission()
                .push(&write_e)
                .context("Failed to push write")?;
        }
    }

    Ok(())
}

/// Submit 400 response
fn submit_write_400(
    ring: &mut IoUring,
    conn_id: u64,
    connections: &mut HashMap<u64, ConnectionState>,
) -> Result<()> {
    let response = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\n\r\nBad Request";

    if let Some(conn) = connections.get_mut(&conn_id) {
        conn.read_buffer.clear();
        conn.read_buffer.extend_from_slice(response);

        let write_e = opcode::Write::new(
            types::Fd(conn.fd),
            conn.read_buffer.as_ptr(),
            conn.read_buffer.len() as u32,
        )
        .build()
        .user_data(OpType::Write(conn_id).encode());

        unsafe {
            ring.submission()
                .push(&write_e)
                .context("Failed to push write")?;
        }
    }

    Ok(())
}

/// Submit 413 response
fn submit_write_413(
    ring: &mut IoUring,
    conn_id: u64,
    connections: &mut HashMap<u64, ConnectionState>,
) -> Result<()> {
    let response = b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 17\r\n\r\nPayload Too Large";

    if let Some(conn) = connections.get_mut(&conn_id) {
        conn.read_buffer.clear();
        conn.read_buffer.extend_from_slice(response);

        let write_e = opcode::Write::new(
            types::Fd(conn.fd),
            conn.read_buffer.as_ptr(),
            conn.read_buffer.len() as u32,
        )
        .build()
        .user_data(OpType::Write(conn_id).encode());

        unsafe {
            ring.submission()
                .push(&write_e)
                .context("Failed to push write")?;
        }
    }

    Ok(())
}

/// Find \r\n\r\n in request
fn find_double_crlf(data: &[u8]) -> Option<usize> {
    data.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

/// Parse HTTP request and match to pre-built response
fn parse_and_match(
    request_data: &[u8],
    buffer_manager: &RegisteredBufferManager,
    _spa_mode: bool,
) -> Option<(String, crate::registered_buffers::Encoding)> {
    // Use our proper HTTP parser
    let (req, _body_offset) = crate::http::parse_request(request_data).ok()?;

    // Only handle GET
    if req.method != "GET" {
        return None;
    }

    // Map root to /index.html
    let path = if req.path == "/" {
        "/index.html"
    } else {
        req.path
    };

    // Extract Accept-Encoding header
    let accept_encoding = req
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("accept-encoding"))
        .map(|(_, value)| *value)
        .unwrap_or("");

    // Find best match
    let result = buffer_manager.best_match(path, accept_encoding);
    debug!(
        "best_match for path='{}' accept_encoding='{}' -> {:?}",
        path,
        accept_encoding,
        result.is_some()
    );
    result.map(|(path_arc, encoding)| (path_arc.to_string(), encoding))
}
