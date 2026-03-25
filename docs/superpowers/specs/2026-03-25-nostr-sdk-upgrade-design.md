# nostr-sdk 0.38 → 0.44 升级设计

**日期**: 2026-03-25
**范围**: `Cargo.toml`, `src/transport.rs`（新建）, `src/nostr.rs`, `src/main.rs`

## 背景

`relay.0xchat.com` 对缺少 `User-Agent` 头的 WebSocket 握手请求返回 HTTP 403 Forbidden。
nostr-sdk 0.38 底层使用 `async-wsocket` → `tokio-tungstenite`，握手请求中没有 `User-Agent`，无法自定义。

nostr-sdk 0.39 起引入了 `WebSocketTransport` trait（位于 `nostr-relay-pool`），允许应用层完全接管 WebSocket 连接建立过程，从而可以注入任意请求头。升级到 0.44.1 后即可实现。

## 目标

1. 升级 nostr-sdk 0.38 → 0.44.1。
2. 新增 `UserAgentTransport`，在握手请求中注入 `User-Agent: rs_tg_nostr/0.1`，解决 403 问题。
3. 迁移 0.38 → 0.44 的 3 处 Breaking API 变更，保持原有功能不变。

## 依赖变更（`Cargo.toml`）

```toml
nostr-sdk         = { version = "0.44.1", features = ["nip59"] }
nostr-relay-pool  = "0.44.0"    # 新增：WebSocketTransport trait
tokio-tungstenite = { version = "0.26", features = ["rustls-tls-webpki-roots"] }  # 新增：自定义握手
async-wsocket     = "0.13"      # 新增：WebSocket enum 构造
```

`nostr-relay-pool`、`tokio-tungstenite`、`async-wsocket` 已在依赖树中（由 nostr-sdk 传递引入），直接声明为直接依赖，Cargo 会统一解析到同一版本，不引入新的 crate。

## 新文件：`src/transport.rs`

### 类型关系

`nostr-relay-pool` 的 `WebSocketTransport` trait 要求返回：

```rust
(WebSocketSink, WebSocketStream)
// 其中：
// WebSocketSink   = Box<dyn Sink<async_wsocket::Message, Error = TransportError> + Send + Unpin>
// WebSocketStream = Pin<Box<dyn Stream<Item = Result<async_wsocket::Message, TransportError>> + Send>>
```

这里的 `Message` 类型是 `async_wsocket::Message`（不是 `tungstenite::Message`）。
`async_wsocket::WebSocket::Tokio(stream)` 内部持有 `tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>`，
且实现了 `Sink<async_wsocket::Message>` 和 `Stream<Item = Result<async_wsocket::Message, async_wsocket::Error>>`，
会在 poll 时自动在 `tungstenite::Message` 和 `async_wsocket::Message` 之间转换。

### 实现步骤

1. 将 `&Url` 转为 `tungstenite::http::Request`，插入 `User-Agent` 头。
2. 调用 `tokio_tungstenite::connect_async_tls_with_config(request, None, false, None)`，
   返回 `WebSocketStream<MaybeTlsStream<TcpStream>>`（与 `async_wsocket::WebSocket::Tokio` 内部类型一致）。
3. 构造 `async_wsocket::WebSocket::Tokio(ws_stream)` 并调用 `.split()` 分离 sink 和 stream。
4. Sink 侧用 newtype 包装（`OurSink`），将 `async_wsocket::Error` → `TransportError`。
   **不使用 `sink_map_err`**（会引起 panic，同 nostr-sdk 官方实现一致）。
5. Stream 侧用 `.map_err(TransportError::backend)` 转换错误类型。

### 不支持的模式

仅支持 `ConnectionMode::Direct`。遇到 Proxy / Tor 时，返回：

```rust
let err = std::io::Error::new(std::io::ErrorKind::Other, "only Direct mode supported");
return Err(TransportError::backend(err));
```

（`&str` 不实现 `std::error::Error`，必须用 `std::io::Error::new` 包装。）

### 结构

```rust
/// Sink newtype（避免 sink_map_err panic）
struct OurSink(SplitSink<WebSocket, AsyncWsMessage>);

impl Sink<AsyncWsMessage> for OurSink {
    type Error = TransportError;
    // poll_ready / start_send / poll_flush / poll_close
    // 每个方法转发给内部 SplitSink，错误用 TransportError::backend 包装
}

#[derive(Debug, Clone, Default)]
pub struct UserAgentTransport;

impl WebSocketTransport for UserAgentTransport {
    fn support_ping(&self) -> bool { true }
    fn connect<'a>(&'a self, url: &'a Url, mode: &'a ConnectionMode, timeout: Duration)
        -> BoxedFuture<'a, Result<(WebSocketSink, WebSocketStream), TransportError>>;
}
```

## `src/nostr.rs` 中的 API 迁移

| 变更点 | 旧 (0.38) | 新 (0.44) |
|--------|-----------|-----------|
| Client 构建 | `Client::new(nostr_keys)` | `Client::builder().signer(nostr_keys).websocket_transport(UserAgentTransport).build()` |
| 订阅过滤器 | `client.subscribe(vec![filter], None)` | `client.subscribe(filter, None)` |
| Gift wrap 解包 | `nip59::extract_rumor(&keys, &event).await` 返回含 `.rumor` 字段的结构体 | `self.client.unwrap_gift_wrap(&event).await` 返回 `UnwrappedGift { sender, rumor }` |
| `RelayMessage` 生命周期 | `RelayMessage`（无生命周期）| `RelayMessage<'static>`（`challenge: Cow<'static, str>`）|

**`RelayMessage::Auth { challenge }` 现在的 `challenge` 类型为 `Cow<'static, str>`**，
但 `challenge.chars().take(16).collect::<String>()` 依然编译通过（`Cow<str>` deref 到 `&str`）。
访问 `gift.rumor.content` 字段名不变。

`send_private_msg` 签名仍接受 `IntoIterator<Item = Tag>`，`std::iter::empty::<Tag>()` 向后兼容。

## `src/main.rs`

新增 `mod transport;`。

## 不变的部分

- `RelayPoolNotification::Message` / `RelayMessage::Auth` 日志逻辑不变（仅 `challenge` 类型略变，调用代码无需修改）。
- `send_private_msg` 调用不变。
- `NostrSender` / `TgSender` trait 不变。
- 所有测试不涉及 `listen()` 或 WebSocket 连接，不受影响。

## 验证

```bash
cargo build    # 无错误、无警告
cargo test     # 全部通过
```

运行验证（需配置 `.env`）：

```bash
RUST_LOG=debug cargo run -- --cwd-dir /tmp/data
```

期望看到（不再出现 403）：

```
INFO  rs_tg_nostr::nostr: Connected to N Nostr relay(s)
DEBUG ...                : WebSocket handshake completed on relay.0xchat.com
```

不应再出现：

```
ERROR ... relay.0xchat.com error=HTTP error: 403 Forbidden
```
