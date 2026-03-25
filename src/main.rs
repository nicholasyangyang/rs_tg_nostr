// src/main.rs
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt};

mod error;
mod config;
mod keys;
mod state;
mod nostr;
mod telegram;
mod app;

/// Telegram ↔ Nostr 消息桥
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// 数据目录（存放 key.json）
    #[arg(long, value_name = "DIR")]
    cwd_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    std::fs::create_dir_all(&cli.cwd_dir)?;

    app::run(cli.cwd_dir).await
}
