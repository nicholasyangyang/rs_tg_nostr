// tests/telegram_test.rs
use axum_test::TestServer;
use serde_json::json;
use std::sync::{Arc, RwLock};

use rs_tg_nostr::state::{AppState, NostrSender, TgSender};
use rs_tg_nostr::error::AppError;
use rs_tg_nostr::config::Config;
use rs_tg_nostr::keys::KeyStore;
use rs_tg_nostr::telegram::webhook_router;
use async_trait::async_trait;
use tempfile::TempDir;

struct MockNostr {
    calls: Arc<RwLock<Vec<(String, String)>>>,
}
impl MockNostr {
    fn new() -> (Self, Arc<RwLock<Vec<(String, String)>>>) {
        let calls = Arc::new(RwLock::new(vec![]));
        (Self { calls: calls.clone() }, calls)
    }
}
#[async_trait]
impl NostrSender for MockNostr {
    async fn send_dm(&self, to_npub: &str, content: &str) -> Result<(), AppError> {
        self.calls.write().unwrap().push((to_npub.to_string(), content.to_string()));
        Ok(())
    }
}

struct MockTg {
    calls: Arc<RwLock<Vec<(i64, String)>>>,
}
impl MockTg {
    fn new() -> (Self, Arc<RwLock<Vec<(i64, String)>>>) {
        let calls = Arc::new(RwLock::new(vec![]));
        (Self { calls: calls.clone() }, calls)
    }
}
#[async_trait]
impl TgSender for MockTg {
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), AppError> {
        self.calls.write().unwrap().push((chat_id, text.to_string()));
        Ok(())
    }
}

#[tokio::test]
async fn test_webhook_allowed_user_sends_dm() {
    let (nostr, nostr_calls) = MockNostr::new();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");
    let keys = Arc::new(KeyStore::load_or_generate(&path).unwrap());
    let (tg, _) = MockTg::new();
    let config = Arc::new(Config {
        bot_token: "token".into(),
        webhook_url: "https://example.com".into(),
        allowed_users: vec![42],
        port: 8000,
        msg_to: "npub1test".to_string(),
        nostr_relays: vec![],
    });
    let state = Arc::new(AppState::new(keys, Arc::new(nostr), Arc::new(tg), config));
    let app = webhook_router(state);
    let server = TestServer::new(app);

    let update = json!({
        "update_id": 1,
        "message": {
            "message_id": 1,
            "from": { "id": 42, "is_bot": false, "first_name": "Alice" },
            "chat": { "id": 100, "type": "private" },
            "text": "hello nostr"
        }
    });

    let resp = server.post("/webhook").json(&update).await;
    resp.assert_status_ok();
    resp.assert_json(&json!({"ok": true}));

    // 等待 tokio spawn 完成
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let calls = nostr_calls.read().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "npub1test");
    assert_eq!(calls[0].1, "hello nostr");
}

#[tokio::test]
async fn test_webhook_blocked_user_returns_ok_no_dm() {
    let (nostr, nostr_calls) = MockNostr::new();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");
    let keys = Arc::new(KeyStore::load_or_generate(&path).unwrap());
    let (tg, _) = MockTg::new();
    let config = Arc::new(Config {
        bot_token: "token".into(),
        webhook_url: "https://example.com".into(),
        allowed_users: vec![42],
        port: 8000,
        msg_to: "npub1test".to_string(),
        nostr_relays: vec![],
    });
    let state = Arc::new(AppState::new(keys, Arc::new(nostr), Arc::new(tg), config));
    let app = webhook_router(state);
    let server = TestServer::new(app);

    let update = json!({
        "update_id": 2,
        "message": {
            "message_id": 2,
            "from": { "id": 99, "is_bot": false, "first_name": "Eve" },
            "chat": { "id": 200, "type": "private" },
            "text": "hack"
        }
    });

    let resp = server.post("/webhook").json(&update).await;
    resp.assert_status_ok();
    resp.assert_json(&json!({"ok": true}));

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let calls = nostr_calls.read().unwrap();
    assert_eq!(calls.len(), 0, "blocked user must not trigger DM");
}

#[tokio::test]
async fn test_webhook_no_text_returns_ok() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");
    let keys = Arc::new(KeyStore::load_or_generate(&path).unwrap());
    let (nostr, nostr_calls) = MockNostr::new();
    let (tg, _) = MockTg::new();
    let config = Arc::new(Config {
        bot_token: "token".into(),
        webhook_url: "https://example.com".into(),
        allowed_users: vec![42],
        port: 8000,
        msg_to: "npub1test".to_string(),
        nostr_relays: vec![],
    });
    let state = Arc::new(AppState::new(keys, Arc::new(nostr), Arc::new(tg), config));
    let app = webhook_router(state);
    let server = TestServer::new(app);

    let update = json!({
        "update_id": 3,
        "message": {
            "message_id": 3,
            "from": { "id": 42, "is_bot": false, "first_name": "Alice" },
            "chat": { "id": 100, "type": "private" }
        }
    });

    let resp = server.post("/webhook").json(&update).await;
    resp.assert_status_ok();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let calls = nostr_calls.read().unwrap();
    assert_eq!(calls.len(), 0);
}
