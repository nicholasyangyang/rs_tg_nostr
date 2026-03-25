# NIP-42 AUTH 日志可见性设计

**日期**: 2026-03-25
**范围**: `src/nostr.rs` — `handle_notifications` 回调

## 背景

nostr-sdk 0.38 默认启用 NIP-42 自动认证（`nip42_auto_authentication: true`）。当 relay 发送 AUTH challenge 时，SDK 自动用客户端私钥签名并回复 kind:22242 事件，整个过程对应用层透明，无任何日志输出。

## 目标

在 relay 发起 AUTH challenge 时打出一条 `info` 级别的日志，便于调试连接问题。

## 设计

在 `NostrBridge::listen()` 的 `handle_notifications` 回调中，新增对 `RelayPoolNotification::Message` 的匹配，过滤出 `RelayMessage::Auth` 消息：

```rust
RelayPoolNotification::Message { relay_url, message } => {
    if let RelayMessage::Auth { challenge } = message {
        info!(
            "NIP-42 AUTH challenge from {} (challenge={}…)",
            relay_url,
            &challenge[..challenge.len().min(16)]
        );
    }
}
```

## 实现细节

- **文件**: `src/nostr.rs`
- **变更**: 在 `handle_notifications` 的 `async move` 块内，在现有 `Event` 分支之后新增 `Message` 分支
- **导入**: 新增 `RelayMessage` 到 `use nostr_sdk::...` 列表
- **不使用** `RelayPoolNotification::Authenticated`（该变体在 0.38 已标 deprecated）
- **日志级别**: `info!`（AUTH 是正常连接流程，不是警告）

## 验证

手动连接需要 NIP-42 的 relay（如 `relay.0xchat.com`），观察日志中出现：
```
INFO rs_tg_nostr::nostr: NIP-42 AUTH challenge from wss://relay.0xchat.com (challenge=abcdef123456…)
```
