use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("key file error: {0}")]
    Keys(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("nostr error: {0}")]
    Nostr(String),

    #[error("telegram error: {0}")]
    Telegram(String),

    #[error("config error: {0}")]
    Config(String),
}

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn test_io_error_converts_to_app_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let app_err: AppError = io_err.into();
        assert!(app_err.to_string().contains("key file error"));
    }

    #[test]
    fn test_nostr_error_display() {
        let e = AppError::Nostr("bad relay".into());
        assert_eq!(e.to_string(), "nostr error: bad relay");
    }
}
