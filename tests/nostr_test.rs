// tests/nostr_test.rs
use nostr_sdk::{Keys, PublicKey, ToBech32};

#[test]
fn test_keys_generate_and_parse() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().unwrap();
    let npub = keys.public_key().to_bech32().unwrap();

    assert!(nsec.starts_with("nsec1"));
    assert!(npub.starts_with("npub1"));

    let rebuilt = Keys::parse(&nsec).unwrap();
    assert_eq!(rebuilt.public_key(), keys.public_key());
}

#[test]
fn test_nip17_keys_valid() {
    let sender = Keys::generate();
    let recipient = Keys::generate();

    let sender_pub: PublicKey = sender.public_key();
    let recipient_pub: PublicKey = recipient.public_key();

    assert_ne!(sender_pub, recipient_pub);
    let npub_str = recipient_pub.to_bech32().unwrap();
    let parsed_pub = PublicKey::parse(&npub_str).unwrap();
    assert_eq!(parsed_pub, recipient_pub);
}
