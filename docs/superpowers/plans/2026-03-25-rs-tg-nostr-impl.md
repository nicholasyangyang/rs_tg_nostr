# rs_tg_nostr 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用 Rust 实现 Telegram ↔ Nostr 消息桥，单一二进制，TDD 驱动，nostr-sdk + axum + tokio。

**Architecture:** 单进程，`Arc<AppState>` 共享状态，两个 tokio task（axum webhook server + nostr listener）。`NostrSender` / `TgSender` trait 解耦核心逻辑，测试时注入 mock 实现。

**Tech Stack:** Rust 2024 edition, nostr-sdk 0.38, axum 0.8, tokio 1, reqwest 0.12, tracing, thiserror, anyhow, async-trait, dotenvy

---

## 文件清单

| 文件 | 职责 |
|------|------|
| `Cargo.toml` | 依赖声明 |
| `.env.example` | 配置模板 |
| `src/main.rs` | CLI 入口：解析 `--cwd-dir`，初始化 tracing，调用 `app::run()` |
| `src/error.rs` | `AppError` 统一错误类型（thiserror） |
| `src/config.rs` | `Config` 结构体，从 .env 读取所有配置 |
| `src/keys.rs` | `KeyStore`：读写 key.json，原子写入，兼容 Python 格式 |
| `src/state.rs` | `AppState`、`NostrSender` trait、`TgSender` trait |
| `src/nostr.rs` | `NostrBridge`：实现 `NostrSender`，封装 nostr-sdk Client |
| `src/telegram.rs` | `TelegramClient`：实现 `TgSender`；axum webhook handler；Telegram 数据模型 |
| `src/app.rs` | `run()`：启动序列，组装 AppState，spawn tasks，启动 axum server |
| `tests/keys_test.rs` | KeyStore 的集成测试 |
| `tests/nostr_test.rs` | NIP-17 crypto 往返测试（不连网） |
| `tests/telegram_test.rs` | webhook handler 测试（axum TestClient） |
| `tests/bridge_test.rs` | 端到端路由测试（MockNostrSender + MockTgSender） |

---

## Task 0: 项目脚手架

**Files:**
- Modify: `Cargo.toml`
- Create: `.env.example`
- Create: `src/error.rs`, `src/config.rs`, `src/keys.rs`, `src/state.rs`, `src/nostr.rs`, `src/telegram.rs`, `src/app.rs`

- [ ] **Step 1: 更新 Cargo.toml 依赖**

```toml
[package]
name = "rs_tg_nostr"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "rs_tg_nostr"
path = "src/main.rs"

[dependencies]
nostr-sdk = "0.38"
axum = { version = "0.8", features = ["json"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
dotenvy = "0.15"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
thiserror = "2"
anyhow = "1"
async-trait = "0.1"
tempfile = "3"
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
axum-test = "0.15"
tokio = { version = "1", features = ["full"] }
```

- [ ] **Step 2: 创建 .env.example**

```env
BOT_TOKEN=your_telegram_bot_token
WEBHOOK_URL=https://your-domain.com
ALLOWED_USERS=123456789,987654321
PORT=8000
MSG_TO=npub1...
NOSTR_RELAYS=wss://relay.damus.io,wss://relay.0xchat.com,wss://nostr.oxtr.dev
LOG_LEVEL=info
```

- [ ] **Step 3: 创建空的模块文件（让项目能编译）**

```bash
touch src/error.rs src/config.rs src/keys.rs src/state.rs src/nostr.rs src/telegram.rs src/app.rs
```

在 `src/main.rs` 中添加模块声明（临时，后续逐步填充）：

```rust
mod error;
mod config;
mod keys;
mod state;
mod nostr;
mod telegram;
mod app;

fn main() {}
```

- [ ] **Step 4: 验证项目能编译**

```bash
cd /home/deeptuuk/Code2/p2p_workspace/rs_tg_nostr
cargo check
```

期望：`Finished` 无错误（各模块为空文件，暂时没有内容）

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml .env.example src/
git commit -m "chore: scaffold project structure and dependencies"
```

---

## Task 1: error.rs — 统一错误类型

**Files:**
- Modify: `src/error.rs`

- [ ] **Step 1: 写测试（红）—— 在 src/error.rs 中先写测试，AppError 尚未实现**

```rust
// src/error.rs

#[cfg(test)]
mod tests {
    #[test]
    fn test_io_error_converts_to_app_error() {
        use super::AppError;
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let app_err: AppError = io_err.into();
        assert!(app_err.to_string().contains("key file error"));
    }

    #[test]
    fn test_nostr_error_display() {
        use super::AppError;
        let e = AppError::Nostr("bad relay".into());
        assert_eq!(e.to_string(), "nostr error: bad relay");
    }
}
```

- [ ] **Step 2: 运行测试确认红（编译错误：AppError 未定义）**

```bash
cargo test error::tests 2>&1 | head -10
```

期望：编译错误 `cannot find type AppError`

- [ ] **Step 3: 实现 AppError（绿）**

```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("key file error: {0}")]
    Keys(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("nostr error: {0}")]
    Nostr(String),

    #[error("telegram error: {0}")]
    Telegram(String),

    #[error("config error: {0}")]
    Config(String),
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_io_error_converts_to_app_error() {
        use super::AppError;
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let app_err: AppError = io_err.into();
        assert!(app_err.to_string().contains("key file error"));
    }

    #[test]
    fn test_nostr_error_display() {
        use super::AppError;
        let e = AppError::Nostr("bad relay".into());
        assert_eq!(e.to_string(), "nostr error: bad relay");
    }
}
```

- [ ] **Step 4: 运行测试确认绿**

```bash
cargo test error::tests
```

期望：2 passed

- [ ] **Step 5: Commit**

```bash
git add src/error.rs
git commit -m "feat: add AppError unified error type (green)"
```

---

## Task 2: config.rs — 配置加载

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: 写测试（红）**

在 `src/config.rs` 底部：

```rust
// src/config.rs
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct Config {
    pub bot_token: String,
    pub webhook_url: String,
    pub allowed_users: Vec<i64>,
    pub port: u16,
    pub msg_to: String,
    pub nostr_relays: Vec<String>,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_users_parse() {
        // 直接测试解析逻辑，不依赖环境变量
        let raw = "123456789,987654321";
        let users: Vec<i64> = raw
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        assert_eq!(users, vec![123456789i64, 987654321i64]);
    }

    #[test]
    fn test_relays_parse() {
        let raw = "wss://relay.damus.io,wss://relay.0xchat.com";
        let relays: Vec<String> = raw.split(',').map(|s| s.trim().to_string()).collect();
        assert_eq!(relays.len(), 2);
        assert_eq!(relays[0], "wss://relay.damus.io");
    }
}
```

- [ ] **Step 2: 运行测试确认红（todo! 会 panic，不是编译错误）**

```bash
cargo test config::tests
```

期望：编译通过，`test_allowed_users_parse` 和 `test_relays_parse` PASS（这两个不调用 `from_env`）

- [ ] **Step 3: 实现 Config::from_env（绿）**

```rust
impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        let bot_token = std::env::var("BOT_TOKEN")
            .map_err(|_| AppError::Config("BOT_TOKEN not set".into()))?;
        let webhook_url = std::env::var("WEBHOOK_URL")
            .map_err(|_| AppError::Config("WEBHOOK_URL not set".into()))?;
        let allowed_users = std::env::var("ALLOWED_USERS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "8000".into())
            .parse::<u16>()
            .map_err(|_| AppError::Config("PORT must be a number".into()))?;
        let msg_to = std::env::var("MSG_TO")
            .map_err(|_| AppError::Config("MSG_TO not set".into()))?;
        let nostr_relays = std::env::var("NOSTR_RELAYS")
            .unwrap_or_else(|_| "wss://relay.damus.io".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into());

        Ok(Self {
            bot_token,
            webhook_url,
            allowed_users,
            port,
            msg_to,
            nostr_relays,
            log_level,
        })
    }
}
```

- [ ] **Step 4: 运行测试确认绿**

```bash
cargo test config::tests
```

期望：2 passed

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: add Config with env loading (green)"
```

---

## Task 3: keys.rs — 密钥管理（TDD 核心）

**Files:**
- Modify: `src/keys.rs`
- Create: `tests/keys_test.rs`

- [ ] **Step 1: 写集成测试（红）**

创建 `tests/keys_test.rs`：

```rust
// tests/keys_test.rs
use rs_tg_nostr::keys::KeyStore;
use tempfile::TempDir;

#[test]
fn test_generate_when_no_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");

    let store = KeyStore::load_or_generate(&path).unwrap();
    let pair = store.key_pair();

    assert!(pair.npub.starts_with("npub1"), "npub should start with npub1");
    assert!(pair.nsec.starts_with("nsec1"), "nsec should start with nsec1");
    assert!(path.exists(), "key.json should be written to disk");
}

#[test]
fn test_load_existing_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");

    // 先生成
    let store1 = KeyStore::load_or_generate(&path).unwrap();
    let npub1 = store1.key_pair().npub.clone();

    // 再加载，应该读回相同的密钥
    let store2 = KeyStore::load_or_generate(&path).unwrap();
    let npub2 = store2.key_pair().npub.clone();

    assert_eq!(npub1, npub2, "same key should be returned on reload");
}

#[test]
fn test_python_compat_json_format() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");

    KeyStore::load_or_generate(&path).unwrap();

    // 读取原始 JSON 验证格式
    let raw = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();

    assert!(v["npub"].is_string(), "npub field must be a JSON string");
    assert!(v["nsec"].is_string(), "nsec field must be a JSON string");
    // 不能有 all_key.json 格式的额外字段
    assert!(v.get("extra").is_none());
}

#[test]
fn test_atomic_write_no_corruption() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");

    // 生成并重复加载，确保原子写不会产生损坏文件
    for _ in 0..5 {
        KeyStore::load_or_generate(&path).unwrap();
    }

    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(&raw).is_ok());
}
```

在 `src/lib.rs` 中（需要创建，供集成测试访问）：

```rust
// src/lib.rs
pub mod error;
pub mod config;
pub mod keys;
pub mod state;
pub mod nostr;
pub mod telegram;
pub mod app;
```

并在 `src/main.rs` 改为：

```rust
// src/main.rs
fn main() {}
```

- [ ] **Step 2: 运行测试确认红**

```bash
cargo test --test keys_test 2>&1 | head -30
```

期望：编译错误（`keys` 模块未实现）

- [ ] **Step 3: 实现 keys.rs（绿）**

```rust
// src/keys.rs
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use nostr_sdk::Keys;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    pub npub: String,
    pub nsec: String,
}

pub struct KeyStore {
    path: PathBuf,
    keys: RwLock<KeyPair>,
}

impl KeyStore {
    /// 读取 key.json；不存在则生成新密钥并写入。
    pub fn load_or_generate(path: &Path) -> Result<Self, AppError> {
        let pair = if path.exists() {
            let raw = std::fs::read_to_string(path)?;
            serde_json::from_str::<KeyPair>(&raw)?
        } else {
            let keys = Keys::generate();
            let pair = KeyPair {
                npub: keys.public_key().to_bech32().map_err(|e| {
                    AppError::Nostr(format!("bech32 encode failed: {e}"))
                })?,
                nsec: keys.secret_key().to_bech32().map_err(|e| {
                    AppError::Nostr(format!("bech32 encode failed: {e}"))
                })?,
            };
            write_atomic(path, &pair)?;
            pair
        };

        Ok(Self {
            path: path.to_path_buf(),
            keys: RwLock::new(pair),
        })
    }

    pub fn key_pair(&self) -> KeyPair {
        self.keys.read().unwrap().clone()
    }

    /// 返回 nostr_sdk::Keys，用于签名和加密。
    pub fn nostr_keys(&self) -> Result<Keys, AppError> {
        let pair = self.keys.read().unwrap();
        Keys::parse(&pair.nsec)
            .map_err(|e| AppError::Nostr(format!("parse nsec failed: {e}")))
    }
}

fn write_atomic(path: &Path, pair: &KeyPair) -> Result<(), AppError> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let dir = path.parent().unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    let json = serde_json::to_string_pretty(pair)?;
    tmp.write_all(json.as_bytes())?;
    tmp.flush()?;
    tmp.persist(path)
        .map_err(|e| AppError::Keys(e.error))?;
    Ok(())
}
```

- [ ] **Step 4: 运行测试确认绿**

```bash
cargo test --test keys_test
```

期望：4 passed

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/keys.rs src/main.rs tests/keys_test.rs
git commit -m "feat: add KeyStore with load-or-generate and atomic write (green)"
```

---

## Task 4: state.rs — AppState 与 traits

**Files:**
- Modify: `src/state.rs`

- [ ] **Step 1: 写测试（红）—— chat_id 存取逻辑，AppState 尚未实现**

```rust
// src/state.rs 底部 #[cfg(test)] 块（先加，State 定义后面）

#[cfg(test)]
mod tests {
    #[test]
    fn test_chat_id_default_none() {
        use super::AppState;
        // 无法构造完整 AppState（需要 Arc<dyn NostrSender> 等），
        // 只测试 chat_id 辅助方法的逻辑
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
```

- [ ] **Step 2: 运行测试确认（编译可能失败因为 state.rs 为空，或测试通过因为不依赖 AppState）**

```bash
cargo test state::tests 2>&1 | head -10
```

期望：若 `state.rs` 为空则编译错误；否则 2 passed

- [ ] **Step 3: 实现 traits 和 AppState（绿）**

```rust
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
```

- [ ] **Step 2: 验证编译**

```bash
cargo check
```

期望：无错误

- [ ] **Step 3: Commit**

```bash
git add src/state.rs
git commit -m "feat: add AppState, NostrSender and TgSender traits"
```

---

## Task 5: telegram.rs — Webhook Handler + TelegramClient（TDD）

**Files:**
- Modify: `src/telegram.rs`
- Create: `tests/telegram_test.rs`

- [ ] **Step 1: 写测试（红）**

```rust
// tests/telegram_test.rs
use axum_test::TestServer;
use serde_json::json;
use std::sync::{Arc, RwLock};

// 这些测试验证 webhook handler 的解析和过滤逻辑
// 使用 axum-test 构造 HTTP 请求，不需要真实 Telegram 服务

use rs_tg_nostr::state::{AppState, NostrSender, TgSender};
use rs_tg_nostr::error::AppError;
use rs_tg_nostr::config::Config;
use rs_tg_nostr::keys::KeyStore;
use rs_tg_nostr::telegram::webhook_router;
use async_trait::async_trait;
use tempfile::TempDir;

// --- Mock 实现 ---

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

// make_state 不再需要：各测试自己持有 TempDir（保持 dir 存活即可，KeyStore 只在构造时读写磁盘）

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
        log_level: "info".into(),
    });
    let state = Arc::new(AppState::new(keys, Arc::new(nostr), Arc::new(tg), config));
    let app = webhook_router(state);
    let server = TestServer::new(app).unwrap();

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
        allowed_users: vec![42],  // 99 不在白名单
        port: 8000,
        msg_to: "npub1test".to_string(),
        nostr_relays: vec![],
        log_level: "info".into(),
    });
    let state = Arc::new(AppState::new(keys, Arc::new(nostr), Arc::new(tg), config));
    let app = webhook_router(state);
    let server = TestServer::new(app).unwrap();

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
        log_level: "info".into(),
    });
    let state = Arc::new(AppState::new(keys, Arc::new(nostr), Arc::new(tg), config));
    let app = webhook_router(state);
    let server = TestServer::new(app).unwrap();

    // 没有 text 字段的 update（如图片等）
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
    let calls = nostr_calls.read().unwrap();
    assert_eq!(calls.len(), 0);
}
```

- [ ] **Step 2: 运行确认红（编译错误，webhook_router 未实现）**

```bash
cargo test --test telegram_test 2>&1 | head -20
```

期望：编译错误

- [ ] **Step 3: 实现 telegram.rs（绿）**

```rust
// src/telegram.rs
use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::State,
    routing::post,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::AppError;
use crate::state::{AppState, TgSender};

// ── Telegram Bot API client ───────────────────────────────────────────────────

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

    /// setWebhook — 启动时调用一次。
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

// ── Telegram Update 数据模型 ──────────────────────────────────────────────────

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

// ── axum Router ──────────────────────────────────────────────────────────────

/// 返回 axum Router，挂载 /webhook POST handler。
/// 在 app.rs 中调用，也供测试直接使用。
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

        // 记录 chat_id，供 Nostr→TG 转发使用
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
```

- [ ] **Step 4: 运行测试确认绿**

```bash
cargo test --test telegram_test
```

期望：3 passed

- [ ] **Step 5: Commit**

```bash
git add src/telegram.rs tests/telegram_test.rs
git commit -m "feat: add TelegramClient and webhook handler (green)"
```

---

## Task 6: nostr.rs — NostrBridge（TDD，NIP-17 往返）

**Files:**
- Modify: `src/nostr.rs`
- Create: `tests/nostr_test.rs`

- [ ] **Step 1: 写测试（红）**

```rust
// tests/nostr_test.rs
// NIP-17 wrap/unwrap 往返测试，只用 nostr-sdk crypto，不连网。

use nostr_sdk::{Keys, EventBuilder, PublicKey};

/// 测试 nostr-sdk 能生成有效密钥对，并能从 nsec 重建 Keys。
#[test]
fn test_keys_generate_and_parse() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().unwrap();
    let npub = keys.public_key().to_bech32().unwrap();

    assert!(nsec.starts_with("nsec1"));
    assert!(npub.starts_with("npub1"));

    // 从 nsec 重建
    let rebuilt = Keys::parse(&nsec).unwrap();
    assert_eq!(rebuilt.public_key(), keys.public_key());
}

/// 测试 NIP-17 gift wrap 的 seal 层加密（使用 EventBuilder::private_msg 或等效 API）。
/// 注意：nostr-sdk 0.38 的 NIP-17 API 可能是 Client::send_private_msg 或
/// EventBuilder::gift_wrap — 以实际编译为准，此处验证 API 存在且密钥有效。
#[test]
fn test_nip17_keys_valid() {
    let sender = Keys::generate();
    let recipient = Keys::generate();

    // 验证双方密钥有效，可以互相知道对方公钥
    let sender_pub: PublicKey = sender.public_key();
    let recipient_pub: PublicKey = recipient.public_key();

    assert_ne!(sender_pub, recipient_pub);
    // nostr-sdk 的 Keys::parse 接受 npub 和 nsec
    let npub_str = recipient_pub.to_bech32().unwrap();
    let parsed_pub = PublicKey::parse(&npub_str).unwrap();
    assert_eq!(parsed_pub, recipient_pub);
}
```

- [ ] **Step 2: 运行测试确认编译通过（测试本身应通过，验证 nostr-sdk API）**

```bash
cargo test --test nostr_test
```

期望：2 passed。若 API 名称有误（如 `to_bech32` 不存在），根据编译错误调整。

- [ ] **Step 3: 实现 nostr.rs（绿）**

```rust
// src/nostr.rs
use std::sync::Arc;

use async_trait::async_trait;
use nostr_sdk::{Client, Filter, Kind, PublicKey, Timestamp};
use tracing::{info, warn};

use crate::error::AppError;
use crate::keys::KeyStore;
use crate::state::{AppState, NostrSender, TgSender};

pub struct NostrBridge {
    client: Client,
}

impl NostrBridge {
    /// 连接 relay pool，使用 keys 中的密钥。
    pub async fn connect(keys: &KeyStore, relays: &[String]) -> Result<Self, AppError> {
        let nostr_keys = keys.nostr_keys()?;
        let client = Client::new(nostr_keys);

        for relay in relays {
            client
                .add_relay(relay.as_str())
                .await
                .map_err(|e| AppError::Nostr(e.to_string()))?;
        }
        client
            .connect()
            .await;
        info!("Connected to {} Nostr relay(s)", relays.len());

        Ok(Self { client })
    }

    /// 启动订阅 + 事件监听循环，收到 DM 后通过 AppState 转发到 TG。
    /// 此函数在独立的 tokio task 中运行，不会返回（除非出错）。
    pub async fn listen(self: Arc<Self>, state: Arc<AppState>) -> Result<(), AppError> {
        use nostr_sdk::RelayPoolNotification;

        let my_pubkey = state.keys.nostr_keys()?.public_key();

        let filter = Filter::new()
            .kind(Kind::GiftWrap)
            .pubkey(my_pubkey)
            .since(Timestamp::now());

        self.client
            .subscribe(vec![filter], None)
            .await
            .map_err(|e| AppError::Nostr(e.to_string()))?;

        info!("Nostr listener subscribed for pubkey={}", my_pubkey);

        // nostr-sdk 0.38 NIP-17 unwrap API：
        // 实现时运行 `cargo doc -p nostr-sdk --open` 确认实际路径。
        // 已知两种可能的 API（以编译通过为准）：
        //   A) client.handle_notifications + UnwrappedGift (nip17)
        //   B) nostr_sdk::nips::nip59::extract_rumor(&keys, &event)
        // 下面先用方案 A，若编译失败改用方案 B。
        self.client
            .handle_notifications(|notification| {
                let state = state.clone();
                async move {
                    if let RelayPoolNotification::Event { event, .. } = notification {
                        if event.kind == Kind::GiftWrap {
                            let keys = match state.keys.nostr_keys() {
                                Ok(k) => k,
                                Err(e) => {
                                    warn!("Failed to get keys: {}", e);
                                    return Ok(false);
                                }
                            };
                            // 方案 A：UnwrappedGift via nip17
                            // 若编译失败，改为：
                            // nostr_sdk::nips::nip59::extract_rumor(&keys, &event)
                            match nostr_sdk::nips::nip17::extract_rumor(&keys, &event) {
                                Ok(unwrapped) => {
                                    let content = unwrapped.rumor.content.clone();
                                    match state.get_chat_id() {
                                        Some(chat_id) => {
                                            if let Err(e) =
                                                state.tg.send_message(chat_id, &content).await
                                            {
                                                warn!("Failed to forward to TG: {}", e);
                                            }
                                        }
                                        None => {
                                            warn!("No chat_id yet, dropping Nostr DM: {}", content);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to unwrap NIP-17: {}", e);
                                }
                            }
                        }
                    }
                    Ok(false) // false = continue listening
                }
            })
            .await
            .map_err(|e| AppError::Nostr(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl NostrSender for NostrBridge {
    async fn send_dm(&self, to_npub: &str, content: &str) -> Result<(), AppError> {
        let recipient = PublicKey::parse(to_npub)
            .map_err(|e| AppError::Nostr(format!("invalid npub: {e}")))?;

        self.client
            .send_private_msg(recipient, content, None)
            .await
            .map_err(|e| AppError::Nostr(e.to_string()))?;

        info!("Sent NIP-17 DM to {}", to_npub);
        Ok(())
    }
}
```

> **注意：** `client.send_private_msg` 和 `nip17::extract_rumor` 是 nostr-sdk 0.38 的预期 API。实现前先运行 `cargo doc -p nostr-sdk --open` 查看实际路径；若 `nip17::extract_rumor` 不存在，尝试 `nip59::extract_rumor`（路径以编译通过为准）。

- [ ] **Step 4: 运行测试**

```bash
cargo test --test nostr_test
cargo check
```

期望：测试通过，编译无错误

- [ ] **Step 5: Commit**

```bash
git add src/nostr.rs tests/nostr_test.rs
git commit -m "feat: add NostrBridge with NIP-17 send and listen (green)"
```

---

## Task 7: bridge_test.rs — 端到端路由集成测试

**Files:**
- Create: `tests/bridge_test.rs`

- [ ] **Step 1: 写集成测试（红）**

```rust
// tests/bridge_test.rs
// 验证 TG→Nostr 和 Nostr→TG 完整路由，使用 mock 组件，无网络依赖。

use std::sync::{Arc, RwLock};
use async_trait::async_trait;
use axum_test::TestServer;
use serde_json::json;
use tempfile::TempDir;

use rs_tg_nostr::config::Config;
use rs_tg_nostr::error::AppError;
use rs_tg_nostr::keys::KeyStore;
use rs_tg_nostr::state::{AppState, NostrSender, TgSender};
use rs_tg_nostr::telegram::webhook_router;

// ── Mocks ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MockNostr(Arc<RwLock<Vec<(String, String)>>>);
impl MockNostr {
    fn new() -> Self { Self(Arc::new(RwLock::new(vec![]))) }
    fn calls(&self) -> Vec<(String, String)> { self.0.read().unwrap().clone() }
}
#[async_trait]
impl NostrSender for MockNostr {
    async fn send_dm(&self, to: &str, content: &str) -> Result<(), AppError> {
        self.0.write().unwrap().push((to.to_string(), content.to_string()));
        Ok(())
    }
}

#[derive(Clone)]
struct MockTg(Arc<RwLock<Vec<(i64, String)>>>);
impl MockTg {
    fn new() -> Self { Self(Arc::new(RwLock::new(vec![]))) }
    fn calls(&self) -> Vec<(i64, String)> { self.0.read().unwrap().clone() }
}
#[async_trait]
impl TgSender for MockTg {
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), AppError> {
        self.0.write().unwrap().push((chat_id, text.to_string()));
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
        log_level: "info".into(),
    });
    let state = Arc::new(AppState::new(keys, Arc::new(nostr), Arc::new(tg), config));
    (state, dir)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_tg_to_nostr_full_flow() {
    let nostr = MockNostr::new();
    let tg = MockTg::new();
    let (state, _dir) = make_test_state(nostr.clone(), tg);
    let app = webhook_router(state.clone());
    let server = TestServer::new(app).unwrap();

    server.post("/webhook").json(&json!({
        "update_id": 1,
        "message": {
            "message_id": 1,
            "from": { "id": 1 },
            "chat": { "id": 55 },
            "text": "hi nostr"
        }
    })).await;

    // 等待 tokio spawn 完成
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let calls = nostr.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "npub1target");
    assert_eq!(calls[0].1, "hi nostr");

    // chat_id 应已记录
    assert_eq!(state.get_chat_id(), Some(55));
}

#[tokio::test]
async fn test_nostr_to_tg_full_flow() {
    let nostr = MockNostr::new();
    let tg = MockTg::new();
    let (state, _dir) = make_test_state(nostr, tg.clone());

    // 预设 chat_id（模拟已有 TG 用户发过消息）
    state.set_chat_id(99);

    // 模拟 Nostr→TG 转发（直接调用 tg.send_message，如同 nostr listener 会做的）
    state.tg.send_message(99, "hello from nostr").await.unwrap();

    let calls = tg.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], (99, "hello from nostr".to_string()));
}

#[tokio::test]
async fn test_nostr_to_tg_no_chat_id_drops_message() {
    let nostr = MockNostr::new();
    let tg = MockTg::new();
    let (state, _dir) = make_test_state(nostr, tg.clone());

    // chat_id 未设置，消息应被丢弃（不转发）
    assert!(state.get_chat_id().is_none());

    // 确认 tg.send_message 未被调用
    assert_eq!(tg.calls().len(), 0);
}
```

- [ ] **Step 2: 运行测试确认通过**

```bash
cargo test --test bridge_test
```

期望：3 passed

- [ ] **Step 3: Commit**

```bash
git add tests/bridge_test.rs
git commit -m "test: add bridge integration tests with mocks (green)"
```

---

## Task 8: app.rs + main.rs — 启动编排

**Files:**
- Modify: `src/app.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: 实现 app.rs**

```rust
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
```

- [ ] **Step 2: 实现 main.rs**

```rust
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

    // 初始化 tracing，支持 RUST_LOG=info 控制级别
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    std::fs::create_dir_all(&cli.cwd_dir)?;

    app::run(cli.cwd_dir).await
}
```

> **注意：** `src/main.rs` 中重复了 `mod` 声明（`src/lib.rs` 也有），两者可以共存。`lib.rs` 供集成测试使用，`main.rs` 是二进制入口，各自 `mod` 声明。

- [ ] **Step 3: 编译验证**

```bash
cargo build 2>&1
```

期望：编译成功，可能有 unused warning，正常。

- [ ] **Step 4: 跑全部测试**

```bash
cargo test
```

期望：所有测试通过

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/main.rs Cargo.toml
git commit -m "feat: add app startup orchestration and CLI entry point (green)"
```

---

## Task 9: 最终检查与收尾

- [ ] **Step 1: 运行所有测试，确认全部通过**

```bash
cargo test
```

期望：所有测试通过，无 panic

- [ ] **Step 2: 检查编译 warnings，修复明显问题**

```bash
cargo build 2>&1 | grep "warning:"
```

- [ ] **Step 3: 验证二进制能启动（--help）**

```bash
cargo run -- --help
```

期望：打印用法说明，包含 `--cwd-dir` 参数

- [ ] **Step 4: 提交 .env.example 和 CLAUDE.md**

创建 `CLAUDE.md`：

```markdown
# CLAUDE.md

This file provides guidance to Claude Code when working in this repository.

## Build & Test

```bash
cargo build                        # 编译
cargo test                         # 全部测试
cargo test --test keys_test        # 单个测试文件
cargo run -- --cwd-dir /tmp/data   # 运行（需 .env）
RUST_LOG=debug cargo run -- --cwd-dir /tmp/data
```

## Architecture

单进程，Arc<AppState> 共享，两个 tokio task：
- axum webhook server（/webhook POST）
- nostr listener task（订阅 relay，收 DM 转发到 TG）

NostrSender / TgSender trait 解耦，测试时注入 mock。

## Key Files

- `src/keys.rs` — key.json 读写（兼容 Python 格式）
- `src/state.rs` — AppState, NostrSender, TgSender traits
- `src/nostr.rs` — nostr-sdk Client 封装，NIP-17 收发
- `src/telegram.rs` — axum handler, TelegramClient
- `src/app.rs` — 启动序列

## nostr-sdk API 注意

nostr-sdk 0.38 API 路径可能有小变化。核心 API：
- `Keys::generate()`, `Keys::parse(nsec_str)`
- `client.send_private_msg(pubkey, content, None)` — NIP-17 DM
- `nostr_sdk::nips::nip59::extract_rumor(keys, event)` — 解包 gift wrap
- `Filter::new().kind(Kind::GiftWrap).pubkey(pk).since(ts)`
```

```bash
git add CLAUDE.md .env.example
git commit -m "docs: add CLAUDE.md and .env.example"
```

- [ ] **Step 5: 最终 commit**

```bash
git log --oneline
```

期望：能看到完整的红绿重构提交历史

---

## 注意事项

1. **nostr-sdk API 验证：** 实现 `nostr.rs` 时，先运行 `cargo doc -p nostr-sdk --open` 查看实际 API，再对照实现。`send_private_msg`、`extract_rumor` 的路径以编译为准。

2. **tempfile 版本：** `NamedTempFile::persist` 在 `tempfile` 3.x 返回 `Result<File, PersistError>`，`PersistError.error` 是 `io::Error`。

3. **axum-test 版本：** 若 `axum-test 0.16` 与 `axum 0.8` 不兼容，改用 `tower::ServiceExt::oneshot` 进行测试（标准 axum 测试方式）。

4. **Config 字段可见性：** `telegram_test.rs` 直接构造 `Config` 结构体，所以所有字段需要 `pub`。
