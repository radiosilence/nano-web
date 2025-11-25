#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use anyhow::Result;
use clap::Parser;
use nano_web::cli;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    cli.run()
}
