mod cli;
mod compression;
mod template;
mod mime_types;
mod fast_routes;
mod ultra_server;
mod axum_server;
mod security;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    
    init_logging(&cli.log_level, &cli.log_format);
    
    cli.run().await
}

fn init_logging(level: &str, format: &str) {
    let env_filter = match level {
        "debug" => "debug",
        "warn" => "warn", 
        "error" => "error",
        _ => "info",
    };
    
    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(env_filter));
        
    if format == "json" {
        subscriber
            .with(tracing_subscriber::fmt::layer())
            .init();
    } else {
        subscriber
            .with(tracing_subscriber::fmt::layer().pretty())
            .init();
    }
}
