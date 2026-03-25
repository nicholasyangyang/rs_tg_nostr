use std::path::{Path, PathBuf};
use std::sync::RwLock;

use nostr_sdk::{Keys, ToBech32};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    pub npub: String,
    pub nsec: String,
}

pub struct KeyStore {
    path: PathBuf,
    keys: RwLock<KeyPair>,
}

impl KeyStore {
    pub fn load_or_generate(path: &Path) -> Result<Self, AppError> {
        let pair = if path.exists() {
            let raw = std::fs::read_to_string(path)?;
            serde_json::from_str::<KeyPair>(&raw)?
        } else {
            let keys = Keys::generate();
            let pair = KeyPair {
                npub: keys.public_key().to_bech32().map_err(|e| {
                    AppError::Nostr(format!("bech32 encode failed: {e}"))
                })?,
                nsec: keys.secret_key().to_bech32().map_err(|e| {
                    AppError::Nostr(format!("bech32 encode failed: {e}"))
                })?,
            };
            write_atomic(path, &pair)?;
            pair
        };

        Ok(Self {
            path: path.to_path_buf(),
            keys: RwLock::new(pair),
        })
    }

    pub fn key_pair(&self) -> KeyPair {
        self.keys.read().unwrap().clone()
    }

    pub fn nostr_keys(&self) -> Result<Keys, AppError> {
        let pair = self.keys.read().unwrap();
        Keys::parse(&pair.nsec)
            .map_err(|e| AppError::Nostr(format!("parse nsec failed: {e}")))
    }
}

fn write_atomic(path: &Path, pair: &KeyPair) -> Result<(), AppError> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let dir = path.parent().unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    let json = serde_json::to_string_pretty(pair)?;
    tmp.write_all(json.as_bytes())?;
    tmp.flush()?;
    tmp.persist(path)
        .map_err(|e| AppError::Keys(e.error))?;
    Ok(())
}
