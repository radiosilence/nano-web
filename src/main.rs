#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use anyhow::Result;
use clap::Parser;
use nano_web::cli;

// On Linux with io_uring, we don't use tokio's runtime
#[cfg(target_os = "linux")]
fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    // Run in blocking context since io_uring has its own runtime
    futures::executor::block_on(cli.run())
}

// On non-Linux, use tokio runtime as before
#[cfg(not(target_os = "linux"))]
#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    cli.run().await
}
