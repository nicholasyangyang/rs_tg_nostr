use rs_tg_nostr::keys::KeyStore;
use tempfile::TempDir;

#[test]
fn test_generate_when_no_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");

    let store = KeyStore::load_or_generate(&path).unwrap();
    let pair = store.key_pair();

    assert!(pair.npub.starts_with("npub1"), "npub should start with npub1");
    assert!(pair.nsec.starts_with("nsec1"), "nsec should start with nsec1");
    assert!(path.exists(), "key.json should be written to disk");
}

#[test]
fn test_load_existing_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");

    let store1 = KeyStore::load_or_generate(&path).unwrap();
    let npub1 = store1.key_pair().npub.clone();

    let store2 = KeyStore::load_or_generate(&path).unwrap();
    let npub2 = store2.key_pair().npub.clone();

    assert_eq!(npub1, npub2, "same key should be returned on reload");
}

#[test]
fn test_python_compat_json_format() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");

    KeyStore::load_or_generate(&path).unwrap();

    let raw = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();

    assert!(v["npub"].is_string(), "npub field must be a JSON string");
    assert!(v["nsec"].is_string(), "nsec field must be a JSON string");
    assert!(v.get("extra").is_none());
}

#[test]
fn test_atomic_write_no_corruption() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("key.json");

    for _ in 0..5 {
        KeyStore::load_or_generate(&path).unwrap();
    }

    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(&raw).is_ok());
}
