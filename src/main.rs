use anyhow::Result;
use clap::Parser;
use nano_web::cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    cli.run().await
}
