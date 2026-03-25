// src/telegram.rs
use std::sync::Arc;

use async_trait::async_trait;
use axum::{Json, Router, extract::State, routing::post};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::AppError;
use crate::state::{AppState, TgSender};

pub struct TelegramClient {
    bot_token: String,
    http: reqwest::Client,
}

impl TelegramClient {
    pub fn new(bot_token: String) -> Self {
        Self {
            bot_token,
            http: reqwest::Client::new(),
        }
    }

    pub async fn register_webhook(&self, webhook_url: &str) -> Result<(), AppError> {
        let url = format!(
            "https://api.telegram.org/bot{}/setWebhook?url={}/webhook",
            self.bot_token, webhook_url
        );
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Telegram(e.to_string()))?;
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Telegram(e.to_string()))?;
        if body["ok"] == true {
            info!("Telegram webhook registered: {}/webhook", webhook_url);
        } else {
            warn!("Telegram webhook registration failed: {:?}", body);
        }
        Ok(())
    }
}

#[async_trait]
impl TgSender for TelegramClient {
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), AppError> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );
        self.http
            .post(&url)
            .json(&serde_json::json!({ "chat_id": chat_id, "text": text }))
            .send()
            .await
            .map_err(|e| AppError::Telegram(e.to_string()))?;
        info!("Sent TG message to chat_id={}", chat_id);
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct TgUpdate {
    pub update_id: i64,
    pub message: Option<TgMessage>,
}

#[derive(Debug, Deserialize)]
pub struct TgMessage {
    pub message_id: i64,
    #[serde(rename = "from")]
    pub from_user: Option<TgUser>,
    pub chat: TgChat,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TgUser {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct TgChat {
    pub id: i64,
}

#[derive(Serialize)]
struct OkResp {
    ok: bool,
}

pub fn webhook_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/webhook", post(webhook_handler))
        .with_state(state)
}

async fn webhook_handler(
    State(state): State<Arc<AppState>>,
    Json(update): Json<TgUpdate>,
) -> Json<OkResp> {
    if let Some(msg) = update.message {
        let Some(text) = msg.text else {
            return Json(OkResp { ok: true });
        };
        let Some(user) = msg.from_user else {
            return Json(OkResp { ok: true });
        };

        if !state.config.allowed_users.contains(&user.id) {
            warn!("Blocked user id={}", user.id);
            return Json(OkResp { ok: true });
        }

        state.set_chat_id(msg.chat.id);

        let nostr = state.nostr.clone();
        let msg_to = state.config.msg_to.clone();
        tokio::spawn(async move {
            if let Err(e) = nostr.send_dm(&msg_to, &text).await {
                warn!("Failed to send Nostr DM: {}", e);
            }
        });
    }

    Json(OkResp { ok: true })
}
