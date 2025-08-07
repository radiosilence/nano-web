use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

const DEFAULT_PORT: u16 = 3000;
const VERSION: &str = include_str!("../VERSION");

#[derive(Parser)]
#[command(name = "nano-web")]
#[command(about = "🔥 Ultra-fast static file server built with Rust")]
#[command(long_about = "🔥 Ultra-fast static file server built with Rust\nRepository: https://github.com/radiosilence/nano-web")]
#[command(version = VERSION)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
    
    #[arg(long = "dir", default_value = "public")]
    #[arg(help = "Directory to serve")]
    pub dir: PathBuf,
    
    #[arg(short = 'p', long = "port", default_value_t = DEFAULT_PORT)]
    #[arg(help = "Port to listen on")]
    pub port: u16,
    
    #[arg(short = 'd', long = "dev")]
    #[arg(help = "Check/reload files if modified")]
    pub dev: bool,
    
    #[arg(long = "spa")]
    #[arg(help = "Enable SPA mode (serve index.html for all routes)")]
    pub spa: bool,
    
    #[arg(long = "config-prefix", default_value = "VITE_")]
    #[arg(help = "Environment variable prefix for config injection")]
    pub config_prefix: String,
    
    #[arg(long = "log-level", default_value = "info")]
    #[arg(help = "Log level (debug, info, warn, error)")]
    pub log_level: String,
    
    #[arg(long = "log-format", default_value = "console")]
    #[arg(help = "Log format (json, console)")]
    pub log_format: String,
    
    #[arg(long = "log-requests", default_value_t = true)]
    #[arg(help = "Log HTTP requests")]
    pub log_requests: bool,
    
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Start the web server")]
    Serve {
        #[arg(help = "Directory to serve")]
        directory: Option<PathBuf>,
        
        #[arg(short = 'p', long = "port")]
        #[arg(help = "Port to listen on")]
        port: Option<u16>,
        
        #[arg(short = 'd', long = "dev")]
        #[arg(help = "Check/reload files if modified")]
        dev: bool,
        
        #[arg(long = "spa")]
        #[arg(help = "Enable SPA mode (serve index.html for all routes)")]
        spa: bool,
        
        #[arg(long = "config-prefix")]
        #[arg(help = "Environment variable prefix for config injection")]
        config_prefix: Option<String>,
        
        #[arg(long = "log-level")]
        #[arg(help = "Log level (debug, info, warn, error)")]
        log_level: Option<String>,
        
        #[arg(long = "log-format")]
        #[arg(help = "Log format (json, console)")]
        log_format: Option<String>,
        
        #[arg(long = "log-requests")]
        #[arg(help = "Log HTTP requests")]
        log_requests: Option<bool>,
        
    },
    #[command(about = "Show version information")]
    Version,
    #[command(about = "Generate completion script")]
    Completion {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Some(Commands::Serve { 
                ref directory, 
                port, 
                dev, 
                spa, 
                config_prefix, 
                log_level: _, 
                log_format: _, 
                log_requests 
            }) => {
                let public_dir = self.dir.clone();
                let serve_dir = directory.clone().unwrap_or(public_dir);
                
                // Use subcommand values or fall back to global defaults
                let final_config = FinalServeConfig {
                    public_dir: serve_dir,
                    port: port.unwrap_or(self.port),
                    dev: dev || self.dev,
                    spa_mode: spa || self.spa,
                    config_prefix: config_prefix.unwrap_or(self.config_prefix),
                    log_requests: log_requests.unwrap_or(self.log_requests),
                };
                
                final_config.serve().await
            }
            Some(Commands::Version) => {
                println!("{}", full_version());
                println!("🔥 Ultra-fast static file server built with Rust");
                println!("Repository: https://github.com/radiosilence/nano-web");
                Ok(())
            }
            Some(Commands::Completion { shell }) => {
                generate_completion(shell);
                Ok(())
            }
            None => {
                // Show help when no subcommand is provided
                let mut cmd = Self::command();
                cmd.print_help()?;
                Ok(())
            }
        }
    }
    
}

struct FinalServeConfig {
    public_dir: PathBuf,
    port: u16,
    dev: bool,
    spa_mode: bool,
    config_prefix: String,
    log_requests: bool,
}

impl FinalServeConfig {
    async fn serve(self) -> Result<()> {
        // Use Axum with our ultra-fast compression and caching system
        let config = crate::axum_server::AxumServeConfig {
            public_dir: self.public_dir,
            port: self.port,
            dev: self.dev,
            spa_mode: self.spa_mode,
            config_prefix: self.config_prefix,
            log_requests: self.log_requests,
        };
        crate::axum_server::start_axum_server(config).await
    }
}

fn full_version() -> String {
    format!("nano-web v{}", VERSION.trim())
}

fn generate_completion(shell: clap_complete::Shell) {
    use clap_complete::generate;
    use std::io;
    
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "nano-web", &mut io::stdout());
}

