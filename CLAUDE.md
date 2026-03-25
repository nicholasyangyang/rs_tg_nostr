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

单进程，`Arc<AppState>` 共享，两个 tokio task：
- axum webhook server（`/webhook` POST）
- nostr listener task（订阅 relay，收 DM 转发到 TG）

`NostrSender` / `TgSender` trait 解耦，测试时注入 mock。

## Key Files

- `src/keys.rs` — key.json 读写（兼容 Python 格式）
- `src/state.rs` — AppState, NostrSender, TgSender traits
- `src/nostr.rs` — nostr-sdk Client 封装，NIP-17 收发
- `src/telegram.rs` — axum handler, TelegramClient
- `src/app.rs` — 启动序列

## nostr-sdk API 注意

nostr-sdk 0.38 核心 API：
- `Keys::generate()`, `Keys::parse(nsec_str)`
- `client.send_private_msg(pubkey, content, std::iter::empty::<Tag>())` — NIP-17 DM
- `nostr_sdk::nips::nip59::extract_rumor(&keys, &event).await` — 解包 gift wrap
- `Filter::new().kind(Kind::GiftWrap).pubkey(pk).since(ts)`
