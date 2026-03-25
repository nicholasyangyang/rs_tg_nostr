# rs_tg_nostr

> English documentation: [README.md](README.md)

用 Rust 编写的 Telegram ↔ Nostr 消息桥。通过 webhook 接收 Telegram 消息，以 NIP-17 Gift Wrap（kind:1059）加密 DM 的形式转发到 Nostr relay；收到 Nostr DM 后转发回 Telegram。

## 功能

- 单一二进制，无需分别启动 gateway 和 CLI 进程
- NIP-17 Gift Wrap（kind:1059）端对端加密 DM
- 兼容 Python `tg-nostr-bot` 的密钥格式（`key.json`）
- 可配置的 Telegram 用户白名单
- 通过 `RUST_LOG` 控制结构化日志

## 依赖

- Rust 1.85+（edition 2024）
- Telegram Bot token（向 @BotFather 申请）
- 可被 Telegram 访问的 HTTPS 端点（TLS 由 nginx/caddy 等反向代理终止，程序本身监听 HTTP）

## 配置

将 `.env.example` 复制到数据目录，命名为 `.env`，填入各项配置：

```env
BOT_TOKEN=your_telegram_bot_token
WEBHOOK_URL=https://your-domain.com
ALLOWED_USERS=123456789,987654321
PORT=8000
MSG_TO=npub1...
NOSTR_RELAYS=wss://relay.damus.io,wss://relay.0xchat.com
LOG_LEVEL=info
```

| 变量 | 说明 |
|------|------|
| `BOT_TOKEN` | Telegram bot token，由 @BotFather 提供 |
| `WEBHOOK_URL` | 公网 HTTPS 基础 URL（程序自动注册 `/webhook`） |
| `ALLOWED_USERS` | 允许发送消息的 Telegram 用户 ID，逗号分隔 |
| `PORT` | HTTP 监听端口（默认 8000） |
| `MSG_TO` | 接收 Telegram 消息的 Nostr npub |
| `NOSTR_RELAYS` | Nostr relay WebSocket 地址，逗号分隔 |
| `LOG_LEVEL` | 日志级别（trace/debug/info/warn/error） |

## 使用方法

```bash
# 编译
cargo build --release

# 运行（从数据目录读取 .env）
./target/release/rs_tg_nostr --cwd-dir ~/bot-data/

# 开启 debug 日志
RUST_LOG=debug ./target/release/rs_tg_nostr --cwd-dir ~/bot-data/
```

首次运行时，程序会自动生成 Nostr 密钥对并写入 `<cwd-dir>/key.json`，同时调用 Telegram `setWebhook` 完成 webhook 注册。

## 密钥文件

密钥存储在 `--cwd-dir` 目录下的 `key.json`，格式如下：

```json
{"npub": "npub1...", "nsec": "nsec1..."}
```

该格式与原 Python `tg-nostr-bot` 完全兼容，可直接复用旧密钥。

## 架构

```
Telegram webhook POST /webhook
        │
   axum handler（Arc<AppState>）
        │ 白名单校验
        │ 记录 chat_id
        └──► NostrBridge::send_dm ──► Nostr relay（NIP-17 kind:1059）

Nostr relay 事件
        │
   nostr listener task
        │ extract_rumor / 解包
        └──► TelegramClient::send_message ──► Telegram 聊天
```

两个 tokio task 通过 `Arc<AppState>` 共享状态。`NostrSender` 和 `TgSender` trait 支持测试时完全替换为 mock 实现。

## 开发

```bash
cargo test                      # 运行所有测试
cargo test --test bridge_test   # 运行单个测试文件
cargo build 2>&1 | grep warning # 检查编译警告
```

## License

MIT
