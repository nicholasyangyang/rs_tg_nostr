# NIP-42 AUTH 日志可见性 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 当 relay 发送 NIP-42 AUTH challenge 时，在日志中打出 `info` 级别记录，方便调试连接问题。

**Architecture:** 在 `NostrBridge::listen()` 的 `handle_notifications` 回调中，把现有的 `if let` 改为 `match`，新增 `Message` 分支过滤 `RelayMessage::Auth`。不改变任何现有行为，只添加日志输出。

**Tech Stack:** nostr-sdk 0.38, tracing, Rust

---

### Task 1: 在 `nostr.rs` 添加 NIP-42 AUTH 日志

**Files:**
- Modify: `src/nostr.rs`

当前 `handle_notifications` 回调使用 `if let RelayPoolNotification::Event { event, .. } = notification`。
需要改为 `match notification { ... }` 以便添加 `Message` 分支。

`RelayPoolNotification::Message` 的 `message` 字段类型是 `RelayMessage`（非 Box，直接匹配），
`RelayMessage::Auth { challenge }` 的 `challenge` 字段类型是 `String`。

- [ ] **Step 1: 在 `use nostr_sdk::...` 中添加 `RelayMessage`**

将 `src/nostr.rs` 顶部的导入从：
```rust
use nostr_sdk::{Client, Filter, Kind, PublicKey, RelayPoolNotification, Timestamp};
```
改为：
```rust
use nostr_sdk::{Client, Filter, Kind, PublicKey, RelayMessage, RelayPoolNotification, Timestamp};
```

- [ ] **Step 2: 将 `if let` 改为 `match`，并添加 `Message` 分支**

将 `handle_notifications` 回调的 `async move` 块内容从：
```rust
async move {
    if let RelayPoolNotification::Event { event, .. } = notification {
        if event.kind == Kind::GiftWrap {
            // ... 现有逻辑 ...
        }
    }
    Ok(false)
}
```
改为：
```rust
async move {
    match notification {
        RelayPoolNotification::Event { event, .. } => {
            if event.kind == Kind::GiftWrap {
                // ... 现有逻辑（完整保留）...
            }
        }
        RelayPoolNotification::Message { relay_url, message } => {
            if let RelayMessage::Auth { challenge } = message {
                info!(
                    "NIP-42 AUTH challenge from {} (challenge={}…)",
                    relay_url,
                    &challenge[..challenge.len().min(16)]
                );
            }
        }
        _ => {}
    }
    Ok(false)
}
```

- [ ] **Step 3: 编译验证**

```bash
cargo build
```
期望输出：`Finished dev profile` 无错误、无警告。

- [ ] **Step 4: 运行全部测试**

```bash
cargo test
```
期望输出：所有测试通过（现有测试不涉及 `listen()`，不受影响）。

- [ ] **Step 5: 提交**

```bash
git add src/nostr.rs
git commit -m "feat: log NIP-42 AUTH challenge via RelayMessage::Auth"
```

- [ ] **Step 6: 推送到远端**

```bash
git push
```
