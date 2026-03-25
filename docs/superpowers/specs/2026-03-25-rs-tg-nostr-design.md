# rs_tg_nostr 设计文档

**日期：** 2026-03-25
**状态：** 已批准
**目标：** 用 Rust 重构 tg-nostr-bot，单一二进制，TDD 驱动，master 风格

---

## 1. 项目概述

`rs_tg_nostr` 是 `tg-nostr-bot` 的 Rust 重写版本。将原有的 Gateway + CLI 两进程架构合并为单一二进制，内部用 tokio task 隔离各功能模块，通过共享 `Arc<AppState>` 通信。

**核心功能（与 Python 版本一致）：**
- Telegram Bot webhook 接收消息
- 通过 NIP-17 Gift Wrap（kind:1059）将 TG 消息发送到 Nostr relay
- 订阅 Nostr relay 收取 DM，转发回 Telegram

---

## 2. 技术选型

| 组件 | 选型 | 理由 |
|------|------|------|
| 异步运行时 | tokio | 生态标准 |
| Nostr 库 | nostr-sdk 0.38 | 原生支持 NIP-17、NIP-44、relay pool |
| Web 框架 | axum 0.8 | tokio 原生，与 nostr-sdk 异步模型契合 |
| HTTP 客户端 | reqwest 0.12 | Telegram Bot API 调用 |
| 日志追踪 | tracing + tracing-subscriber | 支持 RUST_LOG 控制级别 |
| 错误处理 | thiserror（库层）+ anyhow（app 层） | 分层清晰 |
| 配置 | dotenvy | 读取 .env 文件 |

---

## 3. 项目结构

```
rs_tg_nostr/
├── Cargo.toml
├── Cargo.lock
├── .env.example
└── src/
    ├── main.rs          # 入口：解析 --cwd-dir，初始化 tracing，启动 App
    ├── config.rs        # 从 .env 读取配置（BOT_TOKEN、WEBHOOK_URL、ALLOWED_USERS、MSG_TO 等）
    ├── keys.rs          # KeyStore：读写 key.json，兼容 Python {npub, nsec} JSON 格式
    ├── nostr.rs         # NostrBridge + NostrSender trait：relay pool 连接，NIP-17 DM 收发
    ├── telegram.rs      # axum webhook handler + TelegramClient + TgSender trait
    ├── app.rs           # App::run()：组装 AppState，tokio::spawn 各 task，注册 webhook
    └── state.rs         # AppState 定义（Arc 共享）
tests/
    ├── keys_test.rs
    ├── nostr_test.rs
    ├── telegram_test.rs
    └── bridge_test.rs
```

---

## 4. AppState 设计

```rust
pub struct AppState {
    pub keys: Arc<KeyStore>,               // key.json 读写
    pub nostr: Arc<dyn NostrSender>,       // trait object，便于 mock
    pub tg: Arc<dyn TgSender>,             // trait object，便于 mock
    pub config: Arc<Config>,               // BOT_TOKEN、ALLOWED_USERS、MSG_TO 等
    pub chat_id: Arc<RwLock<Option<i64>>>, // 运行时记录最近活跃 chat_id（内存，重启后清空）
}
```

**chat_id 策略：** 纯内存，不持久化。重启后需等待第一条 TG 消息到来才能向 TG 转发 Nostr DM。这与 Python 版本行为一致，可接受。

**可测试 trait：**

```rust
#[async_trait]
pub trait NostrSender: Send + Sync {
    async fn send_dm(&self, to_npub: &str, content: &str) -> Result<(), AppError>;
}

#[async_trait]
pub trait TgSender: Send + Sync {
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), AppError>;
}
```

`NostrBridge` 和 `TelegramClient` 分别实现这两个 trait，测试时注入 mock 实现。

---

## 5. 启动序列（App::run）

```
1. 读取 .env → Config
2. KeyStore::load_or_generate(cwd_dir/key.json)  # 读取或生成 key.json
3. NostrBridge::connect(relays, keys)             # 连接 relay pool
4. TelegramClient::new(bot_token)
5. 组装 AppState(Arc)
6. TelegramClient::register_webhook(webhook_url)  # setWebhook → Telegram API
7. tokio::spawn(nostr_listener_task)              # 订阅 keys.npub 的 kind:1059
8. axum::serve(router, port)                      # 启动 webhook server（阻塞）
```

---

## 6. 数据流

### Telegram → Nostr
```
POST /webhook (Telegram Update JSON)
  → axum handler(Arc<AppState>)
  → 校验 ALLOWED_USERS
  → 记录 chat_id（写入 AppState.chat_id）
  → nostr.send_dm(config.msg_to, text)  # NIP-17 gift wrap → relay
  → 返回 {"ok": true}
```

### Nostr → Telegram
```
nostr-sdk relay 事件循环
  → 收到 kind:1059 gift wrap
  → NIP-17 unwrap（nostr-sdk 内置）→ plaintext
  → 读取 AppState.chat_id
  → 若 chat_id 存在：tg.send_message(chat_id, plaintext)
  → 若 chat_id 为 None：tracing::warn!("no chat_id yet, dropping Nostr DM")
```

### Nostr Listener Task 订阅细节

```rust
// nostr listener task 启动时
let filter = Filter::new()
    .kind(Kind::GiftWrap)             // kind:1059
    .pubkey(keys.public_key())        // #p tag = 本机 npub
    .since(Timestamp::now());         // 只订阅当前时刻之后的新消息
client.subscribe(vec![filter], None).await?;
```

---

## 7. 密钥管理

**KeyStore（keys.rs）：**

```rust
pub struct KeyStore {
    path: PathBuf,
    keys: RwLock<KeyPair>,
}

pub struct KeyPair {
    pub npub: String,   // "npub1..."
    pub nsec: String,   // "nsec1..."
}
```

- 启动时读 `{cwd-dir}/key.json`
- 不存在则 `nostr_sdk::Keys::generate()` 生成并写入
- JSON 格式与 Python 完全兼容：`{"npub": "npub1...", "nsec": "nsec1..."}`
- 写入用原子操作（tempfile + rename），防止写入中断损坏文件
- 只有单一 `key.json`，无 `all_key.json`

---

## 8. 错误处理

```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("key file error: {0}")]
    Keys(#[from] std::io::Error),
    #[error("nostr error: {0}")]
    Nostr(String),                    // 包装 nostr-sdk 错误为 String，避免路径依赖
    #[error("telegram error: {0}")]
    Telegram(String),
    #[error("config missing: {0}")]
    Config(String),
}
```

> nostr-sdk 的错误类型路径在不同版本间可能变化，统一用 `.map_err(|e| AppError::Nostr(e.to_string()))` 包装，避免编译时路径错误。

- axum handler 内部错误 → `tracing::warn!`，返回 `{"ok": true}`（Telegram 不重试）
- Nostr relay 断线 → nostr-sdk 内置自动重连
- 启动失败 → `anyhow::bail!` 打印错误退出

---

## 9. 配置（.env）

```env
BOT_TOKEN=your_telegram_bot_token
WEBHOOK_URL=https://your-domain.com
ALLOWED_USERS=123456789,987654321
PORT=8000
MSG_TO=npub1...
NOSTR_RELAYS=wss://relay.damus.io,wss://relay.0xchat.com
LOG_LEVEL=info
```

启动命令：
```bash
rs_tg_nostr --cwd-dir ~/bot-data/
```

**TLS 说明：** 程序本身监听 HTTP（`0.0.0.0:PORT`）。Telegram webhook 要求 HTTPS，由外部反向代理（nginx、caddy）终止 TLS，程序无需处理证书。

---

## 10. TDD 策略

**原则：** 红→绿→重构，每个循环一次 git commit。

| 测试文件 | 覆盖内容 | mock 策略 |
|----------|----------|-----------|
| `keys_test.rs` | key.json 生成、读取、原子写入、Python 格式兼容 | 使用 `tempfile::TempDir` |
| `nostr_test.rs` | NIP-17 wrap/unwrap 往返（仅 crypto，不连网） | 使用 nostr-sdk 的 `Keys` 生成测试密钥对，`#[ignore]` 标注需联网的 relay 测试 |
| `telegram_test.rs` | webhook JSON 解析，ALLOWED_USERS 过滤，非法请求 | axum `TestClient`（无需真实 TG 服务） |
| `bridge_test.rs` | TG→Nostr 调用链，Nostr→TG 调用链 | 实现 `MockNostrSender` 和 `MockTgSender`（`NostrSender` / `TgSender` trait 的测试实现） |

**Git commit 规范：**
```
feat: add keys module (red)
feat: keys module passing (green)
refactor: keys module
feat: add nostr module (red)
...
```

---

## 11. 与 Python 版本的兼容性

| 项目 | Python 版本 | Rust 版本 |
|------|-------------|-----------|
| key.json 格式 | `{"npub": "...", "nsec": "..."}` | 相同 |
| --cwd-dir 参数 | 必填 | 必填 |
| Nostr relay 协议 | NIP-17 kind:1059 | 相同 |
| WS IPC | Gateway↔CLI WebSocket | 已消除（内部 task） |
| all_key.json | 存在 | 已取消，仅 key.json |
| TLS | 无（依赖外部代理） | 相同 |
| chat_id 持久化 | 无（内存） | 相同（内存） |
