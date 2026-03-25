# rs_tg_nostr

> 中文文档请见 [README_zh.md](README_zh.md)

A Telegram ↔ Nostr message bridge written in Rust. Receives Telegram messages via webhook and forwards them as NIP-17 Gift Wrap DMs (kind:1059) to a Nostr relay; incoming Nostr DMs are forwarded back to Telegram.

## Features

- Single binary — no separate gateway/CLI processes
- NIP-17 Gift Wrap (kind:1059) end-to-end encrypted DMs
- Compatible with Python `tg-nostr-bot` key format (`key.json`)
- Configurable allowed-user allowlist
- Structured logging via `RUST_LOG`

## Requirements

- Rust 1.85+ (edition 2024)
- A Telegram Bot token (`@BotFather`)
- An HTTPS endpoint for the Telegram webhook (nginx/caddy handles TLS; the binary listens plain HTTP)

## Configuration

Copy `.env.example` to `.env` in your data directory and fill in the values:

```env
BOT_TOKEN=your_telegram_bot_token
WEBHOOK_URL=https://your-domain.com
ALLOWED_USERS=123456789,987654321
PORT=8000
MSG_TO=npub1...
NOSTR_RELAYS=wss://relay.damus.io,wss://relay.0xchat.com
LOG_LEVEL=info
```

| Variable | Description |
|----------|-------------|
| `BOT_TOKEN` | Telegram bot token from @BotFather |
| `WEBHOOK_URL` | Public HTTPS base URL (bot registers `/webhook` on it) |
| `ALLOWED_USERS` | Comma-separated Telegram user IDs allowed to send messages |
| `PORT` | HTTP listen port (default: 8000) |
| `MSG_TO` | Nostr npub to send Telegram messages to |
| `NOSTR_RELAYS` | Comma-separated relay WebSocket URLs |
| `LOG_LEVEL` | Log verbosity (trace/debug/info/warn/error) |

## Usage

```bash
# Build
cargo build --release

# Run (reads .env from the data directory)
./target/release/rs_tg_nostr --cwd-dir ~/bot-data/

# With debug logging
RUST_LOG=debug ./target/release/rs_tg_nostr --cwd-dir ~/bot-data/
```

On first run, a Nostr keypair is generated and saved to `<cwd-dir>/key.json`. The binary also calls Telegram's `setWebhook` to register the webhook URL automatically.

## Key File

`key.json` is stored in `--cwd-dir` with the format:

```json
{"npub": "npub1...", "nsec": "nsec1..."}
```

This format is compatible with the original Python `tg-nostr-bot`.

## Architecture

```
Telegram webhook POST /webhook
        │
   axum handler (Arc<AppState>)
        │ allowlist check
        │ record chat_id
        └──► NostrBridge::send_dm  ──► Nostr relay (NIP-17 kind:1059)

Nostr relay event
        │
   nostr listener task
        │ extract_rumor / unwrap
        └──► TelegramClient::send_message ──► Telegram chat
```

Two tokio tasks share state via `Arc<AppState>`. `NostrSender` and `TgSender` traits allow full mock injection in tests.

## Development

```bash
cargo test                      # run all tests
cargo test --test bridge_test   # specific test file
cargo build 2>&1 | grep warning # check for warnings
```

## License

MIT
