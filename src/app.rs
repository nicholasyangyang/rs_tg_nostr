// src/app.rs
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing::info;

use crate::config::Config;
use crate::keys::KeyStore;
use crate::nostr::NostrBridge;
use crate::state::AppState;
use crate::telegram::{TelegramClient, webhook_router};

pub async fn run(cwd_dir: PathBuf) -> Result<()> {
    // 1. 加载 .env（忽略文件不存在）
    let _ = dotenvy::dotenv();

    // 2. 读取配置
    let config = Arc::new(Config::from_env().context("failed to load config")?);

    // 3. 加载或生成密钥
    let key_path = cwd_dir.join("key.json");
    let keys = Arc::new(
        KeyStore::load_or_generate(&key_path).context("failed to load keys")?,
    );
    info!("Loaded keys: npub={}", keys.key_pair().npub);

    // 4. 连接 Nostr relay pool
    let nostr = Arc::new(
        NostrBridge::connect(&keys, &config.nostr_relays)
            .await
            .context("failed to connect to Nostr relays")?,
    );

    // 5. 创建 Telegram client
    let tg = Arc::new(TelegramClient::new(config.bot_token.clone()));

    // 6. 组装 AppState
    let state = Arc::new(AppState::new(keys, nostr.clone(), tg.clone(), config.clone()));

    // 7. 注册 Telegram webhook
    tg.register_webhook(&config.webhook_url)
        .await
        .context("failed to register Telegram webhook")?;

    // 8. 启动 Nostr 监听 task
    let nostr_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = nostr.listen(nostr_state).await {
            tracing::error!("Nostr listener exited: {}", e);
        }
    });

    // 9. 启动 axum server（阻塞直到 Ctrl+C）
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).await.context("failed to bind port")?;
    info!("Listening on {}", addr);

    axum::serve(listener, webhook_router(state)).await.context("server error")?;

    Ok(())
}
