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

## 设计

### 依赖变更（`Cargo.toml`）

```toml
nostr-sdk        = { version = "0.44.1", features = ["nip59"] }
nostr-relay-pool = "0.44.0"           # 新增：WebSocketTransport trait
tokio-tungstenite = { version = "0.26", features = ["rustls-tls-webpki-roots"] }  # 新增：自定义握手
```

`nostr-relay-pool` 和 `tokio-tungstenite` 已在依赖树中（由 nostr-sdk 传递引入），直接声明为直接依赖，不引入新的 crate。

### 新文件：`src/transport.rs`

实现 `UserAgentTransport`，满足 `nostr_relay_pool::transport::websocket::WebSocketTransport` trait。

**核心逻辑**：
1. 将 `Url` 转为 `tungstenite::http::Request`。
2. 插入 `User-Agent: rs_tg_nostr/0.1` 头。
3. 调用 `tokio_tungstenite::connect_async_tls_with_config(request, None, false, None)` 建立连接（自动处理 DNS、TCP、TLS）。
4. 将返回的 `WebSocketStream<MaybeTlsStream<TcpStream>>` split 成 sink 和 stream。
5. Sink 侧用 newtype 包装（不用 `sink_map_err`，防止 panic，与官方 `DefaultWebsocketTransport` 实现保持一致）。
6. 返回 `(WebSocketSink, WebSocketStream)` 符合 trait 要求。

**不支持 Proxy / Tor 模式**（当前项目不使用），收到非 Direct 模式时返回 `TransportError`。

```rust
// 仅支持 Direct 连接
match mode {
    ConnectionMode::Direct => { /* 正常流程 */ }
    _ => return Err(TransportError::backend("only Direct mode is supported")),
}
```

### `src/nostr.rs` 中的 API 迁移

| 变更点 | 旧 (0.38) | 新 (0.44) |
|--------|-----------|-----------|
| Client 构建 | `Client::new(nostr_keys)` | `Client::builder().signer(nostr_keys).websocket_transport(UserAgentTransport).build()` |
| 订阅过滤器 | `client.subscribe(vec![filter], None)` | `client.subscribe(filter, None)` |
| Gift wrap 解包 | `nip59::extract_rumor(&keys, &event).await` 返回 `UnwrappedRumor` | `self.client.unwrap_gift_wrap(&event).await` 返回 `UnwrappedGift { sender, rumor }` |

`rumor.content` 字段名不变，`send_private_msg` 签名向后兼容（仍接受 `IntoIterator<Item = Tag>`）。

`listen()` 中解包后的变量名从 `unwrapped` → `gift`，访问内容改为 `gift.rumor.content`。

### `src/main.rs`

新增 `mod transport;`。

## 不变的部分

- `RelayPoolNotification::Message` / `RelayMessage::Auth` 日志逻辑不变。
- `send_private_msg` 调用不变（`std::iter::empty::<Tag>()` 依然兼容）。
- `NostrSender` / `TgSender` trait 不变。
- 所有测试不涉及 `listen()` 或 WebSocket 连接，不受影响。

## 验证

```bash
cargo build          # 无错误、无警告
cargo test           # 全部通过
```

运行后连接 `wss://relay.0xchat.com`，日志不再出现 `HTTP error: 403 Forbidden`。
