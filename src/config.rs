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
    #[allow(dead_code)]
    pub log_level: String,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_users_parse() {
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
