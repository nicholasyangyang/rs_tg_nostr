// src/state.rs
use std::sync::{Arc, RwLock};

use async_trait::async_trait;

use crate::config::Config;
use crate::error::AppError;
use crate::keys::KeyStore;

/// Nostr DM 发送能力，可被 mock。
#[async_trait]
pub trait NostrSender: Send + Sync {
    async fn send_dm(&self, to_npub: &str, content: &str) -> Result<(), AppError>;
}

/// Telegram 消息发送能力，可被 mock。
#[async_trait]
pub trait TgSender: Send + Sync {
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), AppError>;
}

/// 全局共享状态，用 Arc 传递到所有 task 和 handler。
pub struct AppState {
    pub keys: Arc<KeyStore>,
    pub nostr: Arc<dyn NostrSender>,
    pub tg: Arc<dyn TgSender>,
    pub config: Arc<Config>,
    /// 最近一次 TG 消息的 chat_id，内存存储，重启后清空。
    pub chat_id: Arc<RwLock<Option<i64>>>,
}

impl AppState {
    pub fn new(
        keys: Arc<KeyStore>,
        nostr: Arc<dyn NostrSender>,
        tg: Arc<dyn TgSender>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            keys,
            nostr,
            tg,
            config,
            chat_id: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_chat_id(&self, id: i64) {
        *self.chat_id.write().unwrap() = Some(id);
    }

    pub fn get_chat_id(&self) -> Option<i64> {
        *self.chat_id.read().unwrap()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_chat_id_default_none() {
        use std::sync::{Arc, RwLock};
        let chat_id: Arc<RwLock<Option<i64>>> = Arc::new(RwLock::new(None));
        assert!(chat_id.read().unwrap().is_none());
    }

    #[test]
    fn test_chat_id_set_and_get() {
        use std::sync::{Arc, RwLock};
        let chat_id: Arc<RwLock<Option<i64>>> = Arc::new(RwLock::new(None));
        *chat_id.write().unwrap() = Some(42);
        assert_eq!(*chat_id.read().unwrap(), Some(42));
    }
}
