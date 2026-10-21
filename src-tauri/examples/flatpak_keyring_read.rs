//! Reads the saved passphrases the Flatpak check's app left in its Secret-portal
//! keyring (011 research.md §7, §16; 011 T120).
//!
//! ```sh
//! cargo run --example flatpak_keyring_read -- <default.keyring> <secret>
//! ```
//!
//! `<secret>` is the key the check's stand-in Secret portal handed the app for
//! the run (`scripts/flatpak/openuri_mock.py`), as text. It prints one JSON array of
//! `{ "attributes": {...}, "secret": "..." }`, one object per item, and fails (exit 1,
//! a reason on standard error) when the file is missing or the key doesn't open it.
//! It reads the file and nothing else: it makes no connection to a bus and never
//! touches a keyring of the computer it runs on.

#[cfg(target_os = "linux")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [file, secret] = args.as_slice() else {
        eprintln!("usage: flatpak_keyring_read <default.keyring> <secret>");
        std::process::exit(2);
    };
    match read(file, secret) {
        Ok(json) => println!("{json}"),
        Err(reason) => {
            eprintln!("flatpak_keyring_read: {reason}");
            std::process::exit(1);
        }
    }
}

#[cfg(target_os = "linux")]
fn read(file: &str, secret: &str) -> Result<String, String> {
    use oo7::Secret;
    use oo7::file::UnlockedKeyring;

    if !std::path::Path::new(file).is_file() {
        return Err(format!("{file} is not a file"));
    }
    zbus::block_on(async {
        let keyring = UnlockedKeyring::load(file, Secret::blob(secret.as_bytes()))
            .await
            .map_err(|err| format!("the key does not open {file}: {err}"))?;
        let mut entries = Vec::new();
        for item in keyring.items().await.map_err(|err| err.to_string())? {
            let item = item.map_err(|err| format!("an item does not decrypt: {err}"))?;
            let item = item.as_unlocked();
            entries.push(serde_json::json!({
                "attributes": item.attributes(),
                "secret": String::from_utf8_lossy(item.secret().as_bytes()),
            }));
        }
        Ok(serde_json::Value::Array(entries).to_string())
    })
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("flatpak_keyring_read reads a Flatpak's keyring file: Linux only");
}
