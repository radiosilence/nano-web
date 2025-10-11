#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use anyhow::Result;
use clap::Parser;
use nano_web::cli;

// On Linux with io_uring, we use tokio_uring which has its own runtime
// No #[tokio::main] needed - tokio_uring::builder().start() is blocking
// but we still need to block_on the async cli.run() function
#[cfg(target_os = "linux")]
fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    futures::executor::block_on(cli.run())
}

// On non-Linux, use tokio runtime as before
#[cfg(not(target_os = "linux"))]
#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    cli.run().await
}
