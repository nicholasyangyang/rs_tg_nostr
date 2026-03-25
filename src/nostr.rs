// src/nostr.rs
use std::sync::Arc;

use async_trait::async_trait;
use nostr_sdk::{Client, Filter, Kind, PublicKey, RelayPoolNotification, Timestamp};
use tracing::{info, warn};

use crate::error::AppError;
use crate::keys::KeyStore;
use crate::state::{AppState, NostrSender};

pub struct NostrBridge {
    client: Client,
}

impl NostrBridge {
    pub async fn connect(keys: &KeyStore, relays: &[String]) -> Result<Self, AppError> {
        let nostr_keys = keys.nostr_keys()?;
        let client = Client::new(nostr_keys);

        for relay in relays {
            client
                .add_relay(relay.as_str())
                .await
                .map_err(|e| AppError::Nostr(e.to_string()))?;
        }
        client.connect().await;
        info!("Connected to {} Nostr relay(s)", relays.len());

        Ok(Self { client })
    }

    pub async fn listen(self: Arc<Self>, state: Arc<AppState>) -> Result<(), AppError> {
        let my_pubkey = state.keys.nostr_keys()?.public_key();

        // NIP-59 gift wrap events have intentionally backdated created_at (up to 48h in the
        // past) to prevent timing correlation. Using since(now) would drop all incoming DMs.
        let since = Timestamp::now() - 2 * 24 * 60 * 60;
        let filter = Filter::new()
            .kind(Kind::GiftWrap)
            .pubkey(my_pubkey)
            .since(since);

        self.client
            .subscribe(vec![filter], None)
            .await
            .map_err(|e| AppError::Nostr(e.to_string()))?;

        info!("Nostr listener subscribed for pubkey={}", my_pubkey);

        self.client
            .handle_notifications(|notification| {
                let state = state.clone();
                async move {
                    if let RelayPoolNotification::Event { event, .. } = notification {
                        if event.kind == Kind::GiftWrap {
                            let keys = match state.keys.nostr_keys() {
                                Ok(k) => k,
                                Err(e) => {
                                    warn!("Failed to get keys: {}", e);
                                    return Ok(false);
                                }
                            };
                            // NIP-59 gift wrap 解包，使用 async extract_rumor
                            match nostr_sdk::nips::nip59::extract_rumor(&keys, &event).await {
                                Ok(unwrapped) => {
                                    let content = unwrapped.rumor.content.clone();
                                    match state.get_chat_id() {
                                        Some(chat_id) => {
                                            if let Err(e) =
                                                state.tg.send_message(chat_id, &content).await
                                            {
                                                warn!("Failed to forward to TG: {}", e);
                                            }
                                        }
                                        None => {
                                            warn!("No chat_id yet, dropping Nostr DM");
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to unwrap NIP-17: {}", e);
                                }
                            }
                        }
                    }
                    Ok(false)
                }
            })
            .await
            .map_err(|e| AppError::Nostr(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl NostrSender for NostrBridge {
    async fn send_dm(&self, to_npub: &str, content: &str) -> Result<(), AppError> {
        let recipient = PublicKey::parse(to_npub)
            .map_err(|e| AppError::Nostr(format!("invalid npub: {e}")))?;

        self.client
            .send_private_msg(recipient, content, std::iter::empty::<nostr_sdk::Tag>())
            .await
            .map_err(|e| AppError::Nostr(e.to_string()))?;

        info!("Sent NIP-17 DM to {}", to_npub);
        Ok(())
    }
}
