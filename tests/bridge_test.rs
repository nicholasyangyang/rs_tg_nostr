// tests/bridge_test.rs
use axum_test::TestServer;
use serde_json::json;
use std::sync::{Arc, RwLock};

use rs_tg_nostr::config::Config;
use rs_tg_nostr::error::AppError;
use rs_tg_nostr::keys::KeyStore;
use rs_tg_nostr::state::{AppState, NostrSender, TgSender};
use rs_tg_nostr::telegram::webhook_router;
use async_trait::async_trait;
use tempfile::TempDir;

#[derive(Clone)]
struct MockNostr {
    calls: Arc<RwLock<Vec<(String, String)>>>,
}
impl MockNostr {
    fn new() -> Self {
        Self {
            calls: Arc::new(RwLock::new(vec![])),
        }
    }
    fn get_calls(&self) -> Vec<(String, String)> {
        self.calls.read().unwrap().clone()
    }
}
#[async_trait]
impl NostrSender for MockNostr {
    async fn send_dm(&self, to: &str, content: &str) -> Result<(), AppError> {
        self.calls.write().unwrap().push((to.to_string(), content.to_string()));
        Ok(())
    }
}

#[derive(Clone)]
struct MockTg {
    calls: Arc<RwLock<Vec<(i64, String)>>>,
}
impl MockTg {
    fn new() -> Self {
        Self {
            calls: Arc::new(RwLock::new(vec![])),
        }
    }
    fn get_calls(&self) -> Vec<(i64, String)> {
        self.calls.read().unwrap().clone()
    }
}
#[async_trait]
impl TgSender for MockTg {
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), AppError> {
        self.calls.write().unwrap().push((chat_id, text.to_string()));
        Ok(())
    }
}

fn make_test_state(nostr: MockNostr, tg: MockTg) -> (Arc<AppState>, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");
    let keys = Arc::new(KeyStore::load_or_generate(&path).unwrap());
    let config = Arc::new(Config {
        bot_token: "tok".into(),
        webhook_url: "https://example.com".into(),
        allowed_users: vec![1],
        port: 8000,
        msg_to: "npub1target".into(),
        nostr_relays: vec![],
    });
    let state = Arc::new(AppState::new(keys, Arc::new(nostr), Arc::new(tg), config));
    (state, dir)
}

#[tokio::test]
async fn test_tg_to_nostr_full_flow() {
    let nostr = MockNostr::new();
    let tg = MockTg::new();
    let (state, _dir) = make_test_state(nostr.clone(), tg);
    let app = webhook_router(state.clone());
    let server = TestServer::new(app);

    let update = json!({
        "update_id": 1,
        "message": {
            "message_id": 1,
            "from": { "id": 1 },
            "chat": { "id": 55 },
            "text": "hi nostr"
        }
    });

    let resp = server.post("/webhook").json(&update).await;
    resp.assert_status_ok();
    resp.assert_json(&json!({"ok": true}));

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let calls = nostr.get_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "npub1target");
    assert_eq!(calls[0].1, "hi nostr");

    assert_eq!(state.get_chat_id(), Some(55));
}

#[tokio::test]
async fn test_nostr_to_tg_full_flow() {
    let nostr = MockNostr::new();
    let tg = MockTg::new();
    let (state, _dir) = make_test_state(nostr, tg.clone());

    state.set_chat_id(99);
    state.tg.send_message(99, "hello from nostr").await.unwrap();

    let calls = tg.get_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], (99, "hello from nostr".to_string()));
}

#[tokio::test]
async fn test_nostr_to_tg_no_chat_id_does_not_send() {
    let nostr = MockNostr::new();
    let tg = MockTg::new();
    let (state, _dir) = make_test_state(nostr, tg.clone());

    assert!(state.get_chat_id().is_none());
    assert_eq!(tg.get_calls().len(), 0);
}
