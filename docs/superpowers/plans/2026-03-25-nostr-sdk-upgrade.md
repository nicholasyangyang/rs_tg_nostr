# nostr-sdk 0.38 → 0.44 升级 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 升级 nostr-sdk 0.38 → 0.44.1，通过实现自定义 `UserAgentTransport` 注入 `User-Agent` 请求头，修复 `relay.0xchat.com` 返回 HTTP 403 的问题。

**Architecture:** 新增 `src/transport.rs` 实现 `WebSocketTransport` trait，内部通过 `tokio_tungstenite::connect_async_tls_with_config` 建立带自定义头的 WS 连接，再包装为 `async_wsocket::WebSocket::Tokio(stream)` 以满足 nostr-relay-pool 的类型要求。同时在 `src/nostr.rs` 中迁移 3 处 0.38→0.44 Breaking API 变更。

**Tech Stack:** nostr-sdk 0.44.1, nostr-relay-pool 0.44.0, tokio-tungstenite 0.26, async-wsocket 0.13, Rust

---

### 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `Cargo.toml` | Modify | 升级/新增依赖 |
| `src/transport.rs` | Create | `UserAgentTransport` + `OurSink` newtype |
| `src/nostr.rs` | Modify | API 迁移：ClientBuilder、subscribe、unwrap_gift_wrap |
| `src/main.rs` | Modify | 新增 `mod transport;` |

---

### Task 1: 升级依赖（`Cargo.toml`）

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 更新 `Cargo.toml`**

将 `[dependencies]` 中的 nostr-sdk 行改为：

```toml
nostr-sdk         = { version = "0.44.1", features = ["nip59"] }
nostr-relay-pool  = "0.44.0"
tokio-tungstenite = { version = "0.26", features = ["rustls-tls-webpki-roots"] }
async-wsocket     = "0.13"
```

`nostr-sdk` 那行原文是 `nostr-sdk = { version = "0.38", features = ["nip59"] }`，替换整行。其余三行在 `nostr-sdk` 行之后新增。

- [ ] **Step 2: 验证依赖可解析**

```bash
cargo fetch
```

期望输出：下载 nostr-sdk 0.44.1、nostr-relay-pool 0.44.0 等，无错误。

- [ ] **Step 3: 提交**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: upgrade nostr-sdk 0.38 -> 0.44.1, add transport deps"
```

---

### Task 2: 新建 `src/transport.rs`

**Files:**
- Create: `src/transport.rs`

无现有测试覆盖 WebSocket 连接，编译通过即为验证。

- [ ] **Step 1: 创建 `src/transport.rs`**

写入以下完整内容：

```rust
// src/transport.rs
use std::fmt;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use async_wsocket::futures_util::stream::SplitSink;
use async_wsocket::futures_util::{Sink, SinkExt, StreamExt};
use async_wsocket::{ConnectionMode, Message as AsyncWsMessage, WebSocket};
use nostr::util::BoxedFuture;
use nostr::Url;
use nostr_relay_pool::transport::websocket::{
    TransportError, WebSocketSink, WebSocketStream, WebSocketTransport,
};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

const USER_AGENT: &str = concat!("rs_tg_nostr/", env!("CARGO_PKG_VERSION"));

// Newtype wrapper — do NOT use sink_map_err (can cause panics, see rust-nostr#984)
struct OurSink(SplitSink<WebSocket, AsyncWsMessage>);

impl fmt::Debug for OurSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OurSink").finish()
    }
}

impl Sink<AsyncWsMessage> for OurSink {
    type Error = TransportError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.0)
            .poll_ready(cx)
            .map_err(TransportError::backend)
    }

    fn start_send(mut self: Pin<&mut Self>, item: AsyncWsMessage) -> Result<(), Self::Error> {
        Pin::new(&mut self.0)
            .start_send(item)
            .map_err(TransportError::backend)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.0)
            .poll_flush(cx)
            .map_err(TransportError::backend)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.0)
            .poll_close(cx)
            .map_err(TransportError::backend)
    }
}

/// WebSocket transport that injects a `User-Agent` header into the handshake
/// request, fixing HTTP 403 on relays that require it (e.g. relay.0xchat.com).
///
/// Only supports `ConnectionMode::Direct`.
#[derive(Debug, Clone, Default)]
pub struct UserAgentTransport;

impl WebSocketTransport for UserAgentTransport {
    fn support_ping(&self) -> bool {
        true
    }

    fn connect<'a>(
        &'a self,
        url: &'a Url,
        mode: &'a ConnectionMode,
        _timeout: Duration,
    ) -> BoxedFuture<'a, Result<(WebSocketSink, WebSocketStream), TransportError>> {
        Box::pin(async move {
            if !matches!(mode, ConnectionMode::Direct) {
                return Err(TransportError::backend(io::Error::new(
                    io::ErrorKind::Other,
                    "UserAgentTransport only supports Direct mode",
                )));
            }

            let mut request = url
                .as_str()
                .into_client_request()
                .map_err(TransportError::backend)?;
            request.headers_mut().insert(
                "User-Agent",
                HeaderValue::from_static(USER_AGENT),
            );

            let (ws_stream, _response) =
                tokio_tungstenite::connect_async_tls_with_config(request, None, false, None)
                    .await
                    .map_err(TransportError::backend)?;

            // Wrap as async-wsocket WebSocket so Message types align
            let socket = WebSocket::Tokio(ws_stream);
            let (tx, rx) = socket.split();

            let sink: WebSocketSink = Box::new(OurSink(tx));
            let stream: WebSocketStream =
                Box::pin(rx.map_err(TransportError::backend));

            Ok((sink, stream))
        })
    }
}
```

- [ ] **Step 2: 在 `src/main.rs` 中注册模块**

在 `src/main.rs` 中，在现有的 `mod error;` 等行附近添加：

```rust
mod transport;
```

- [ ] **Step 3: 编译验证**

```bash
cargo build 2>&1 | head -40
```

期望：`Finished dev profile` 无错误。如有错误，根据编译器提示修正（常见：`Unpin` 约束、import 路径）。

- [ ] **Step 4: 提交**

```bash
git add src/transport.rs src/main.rs
git commit -m "feat: add UserAgentTransport for custom WS User-Agent header"
```

---

### Task 3: 迁移 `src/nostr.rs` API

**Files:**
- Modify: `src/nostr.rs`

这是 3 处 Breaking API 变更的集中迁移。

- [ ] **Step 1: 更新 `use` 导入**

将文件顶部的 `use nostr_sdk::...` 行改为：

```rust
use nostr_sdk::{Client, Filter, Kind, PublicKey, RelayMessage, RelayPoolNotification, Timestamp, UnwrappedGift};
```

（删去 `Tag`，新增 `UnwrappedGift`；其余不变。）

同时在文件顶部新增：

```rust
use crate::transport::UserAgentTransport;
```

- [ ] **Step 2: 修改 `NostrBridge::connect` — 使用 `ClientBuilder` + `UserAgentTransport`**

将：

```rust
let client = Client::new(nostr_keys);
```

改为：

```rust
let client = Client::builder()
    .signer(nostr_keys)
    .websocket_transport(UserAgentTransport)
    .build();
```

- [ ] **Step 3: 修改 `subscribe` 调用 — 去掉 `vec![]` 包装**

将：

```rust
self.client
    .subscribe(vec![filter], None)
    .await
    .map_err(|e| AppError::Nostr(e.to_string()))?;
```

改为：

```rust
self.client
    .subscribe(filter, None)
    .await
    .map_err(|e| AppError::Nostr(e.to_string()))?;
```

- [ ] **Step 4: 迁移 `extract_rumor` → `unwrap_gift_wrap`**

在 `handle_notifications` 回调的 `RelayPoolNotification::Event` 分支中，将：

```rust
let keys = match state.keys.nostr_keys() {
    Ok(k) => k,
    Err(e) => {
        warn!("Failed to get keys: {}", e);
        return Ok(false);
    }
};
// NIP-59 gift wrap 解包，使用 async extract_rumor
match nostr_sdk::nips::nip59::extract_rumor(&keys, &event).await {
    Ok(unwrapped) => {
        let content = unwrapped.rumor.content.clone();
```

替换为：

```rust
match self.client.unwrap_gift_wrap(&event).await {
    Ok(gift) => {
        let content = gift.rumor.content.clone();
```

（删去整个 `let keys = match ...` 块，`unwrap_gift_wrap` 内部自动从 client signer 获取密钥。`content` 访问路径 `.rumor.content` 不变。）

同时 `listen` 函数签名中的 `self: Arc<Self>` 保持不变，但内部现在用 `self.client.unwrap_gift_wrap(...)` — 需确认 `self` 可访问（它通过 `let state = state.clone(); async move { ... }` closure 捕获，`self` 需要在 closure 外先 clone 或通过 `Arc` 传入）。

具体地：在 `handle_notifications` 回调 `|notification| { let state = state.clone(); async move { ... } }` 的 closure 开头，已有 `let state = state.clone()`。还需在同一位置加一行：

```rust
let client = Arc::clone(&self.client);  // 在 handle_notifications 调用之前
```

然后在 closure 内部改用 `client.unwrap_gift_wrap(&event).await`。

完整修改后的 `listen` 中 `handle_notifications` 调用如下：

```rust
let client = Arc::clone(&self.client);
self.client
    .handle_notifications(|notification| {
        let state = state.clone();
        let client = client.clone();
        async move {
            match notification {
                RelayPoolNotification::Event { event, .. } => {
                    if event.kind == Kind::GiftWrap {
                        match client.unwrap_gift_wrap(&event).await {
                            Ok(gift) => {
                                let content = gift.rumor.content.clone();
                                match state.get_chat_id() {
                                    Some(chat_id) => {
                                        if let Err(e) =
                                            state.tg.send_message(chat_id, &content).await
                                        {
                                            warn!("Failed to forward to TG: {}", e);
                                        }
                                    }
                                    None => {
                                        warn!("No chat_id yet, dropping Nostr DM");
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to unwrap NIP-17: {}", e);
                            }
                        }
                    }
                }
                RelayPoolNotification::Message { relay_url, message } => {
                    if let RelayMessage::Auth { challenge } = message {
                        let preview: String = challenge.chars().take(16).collect();
                        info!(
                            "NIP-42 AUTH challenge from {} (challenge={}…)",
                            relay_url,
                            preview
                        );
                    }
                }
                _ => {}
            }
            Ok(false)
        }
    })
    .await
    .map_err(|e| AppError::Nostr(e.to_string()))?;
```

- [ ] **Step 5: 更新 `NostrSender::send_dm` 中的 Tag import**

`send_dm` 中用到 `nostr_sdk::Tag`。由于顶部 `use` 已删去 `Tag`，将其改为显式路径：

```rust
self.client
    .send_private_msg(recipient, content, std::iter::empty::<nostr_sdk::Tag>())
    .await
    .map_err(|e| AppError::Nostr(e.to_string()))?;
```

（即在 `empty::<>` 的泛型参数中用完整路径，无需 import `Tag`。）

- [ ] **Step 6: 编译验证**

```bash
cargo build
```

期望：`Finished dev profile` 无错误、无警告。

- [ ] **Step 7: 运行全部测试**

```bash
cargo test
```

期望：所有测试通过（现有测试使用 mock，不涉及 WebSocket 连接，不受影响）。

- [ ] **Step 8: 提交**

```bash
git add src/nostr.rs
git commit -m "feat: migrate nostr.rs to nostr-sdk 0.44 API with UserAgentTransport"
```

---

### Task 4: 运行时验证

**Files:** 无代码改动，仅运行时检查。

- [ ] **Step 1: 准备 `.env`**

确认项目根目录有 `.env`（参考 `.env.example`），含有效的 `BOT_TOKEN`、`NOSTR_RELAYS`（含 `wss://relay.0xchat.com`）、`WEBHOOK_URL` 等。

- [ ] **Step 2: 启动并观察日志**

```bash
RUST_LOG=debug cargo run -- --cwd-dir /tmp/tg_nostr_data
```

**期望看到（成功）：**

```
INFO  rs_tg_nostr::nostr: Connected to N Nostr relay(s)
INFO  rs_tg_nostr::nostr: Nostr listener subscribed for pubkey=...
```

**不应再出现：**

```
ERROR ... relay.0xchat.com error=HTTP error: 403 Forbidden
```

- [ ] **Step 3: 提交运行结果确认（无代码改动）**

若日志正常，在 GitHub 上记录测试通过。若仍出现 403，检查 `UserAgentTransport` 是否通过 `websocket_transport()` 正确注册。

---

### Task 5: 最终清理与推送

**Files:** 无新增代码。

- [ ] **Step 1: 检查是否有遗留 warning**

```bash
cargo build 2>&1 | grep "warning"
```

若有 `dead_code` 或 `unused_import`，清理之。

- [ ] **Step 2: 推送到远端**

```bash
git push
```
