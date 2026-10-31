#![cfg(target_os = "linux")]
//! Saved passphrases inside a Flatpak, through the Secret portal (011 FR-006,
//! 011 US2-5; 011 research.md §7; 011 data-model.md "Saved passphrases inside a
//! Flatpak").
//!
//! The store is real: oo7's file keyring in a temp folder, encrypted with a test
//! secret that a stand-in [`SecretPortal`] supplies (the production source is the
//! portal's `RetrieveSecret`, which the Flatpak check covers with its own
//! stand-in portal). Availability is real too: each test starts its own
//! `dbus-daemon` on a private address (never the developer's session bus) and
//! hands `secret_portal_available` a connection to it. `dbus-daemon` is in the dev
//! container; a missing one fails the test with a message naming it.
//!
//! The default keyring store (keyring-core's `set_default_store`) is not touched,
//! so these tests run in the ordinary Rust run beside the others.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::support::private_bus::Bus;
use hoplodex_lib::services::keyring::Keyring;
use hoplodex_lib::services::keyring::portal::{SecretPortal, secret_portal_available};
use hoplodex_lib::services::passphrase::Passphrase;
use tempfile::TempDir;
use zbus::zvariant::{OwnedFd, OwnedValue};

const SERVICE: &str = "io.github.exodious.HoploDex";
const SECRET: &[u8] = b"a 64-byte test key that stands in for the Secret portal's per-app secret";
const OTHER_SECRET: &[u8] = b"another key, as a different app id or a reset keyring would give";

// ------------------------------------------------------------------ the store

/// A portal with a fixed secret that counts the times it is asked for it.
struct FixedPortal {
    secret: Vec<u8>,
    available: bool,
    asked: AtomicUsize,
}

impl FixedPortal {
    fn with(secret: &[u8]) -> Arc<Self> {
        Arc::new(Self { secret: secret.to_vec(), available: true, asked: AtomicUsize::new(0) })
    }

    fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }
}

impl SecretPortal for FixedPortal {
    fn is_available(&self) -> bool {
        self.available
    }

    fn retrieve_secret(&self) -> Result<Vec<u8>, String> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(self.secret.clone())
    }
}

/// A keyring file in a folder that does not exist yet, as the Flatpak's data
/// folder has no `keyrings` in it on a first run.
fn keyring_file(dir: &TempDir) -> PathBuf {
    dir.path().join("data").join("keyrings").join("default.keyring")
}

fn typed(text: &str) -> Passphrase {
    Passphrase::from_input(text.to_owned())
}

/// 011 FR-006: saving, reading and forgetting a passphrase work through the
/// portal's store, with the names every install uses, and survive a relaunch.
#[test]
fn save_read_and_forget_round_trip_through_the_portal_store() {
    let dir = TempDir::new().unwrap();
    let file = keyring_file(&dir);
    let portal = FixedPortal::with(SECRET);
    let keyring = Keyring::portal(file.clone(), portal.clone());

    assert!(keyring.is_available());
    assert_eq!(portal.asked(), 0, "availability must not ask for the secret (RetrieveSecret)");
    assert!(keyring.load("db-1").is_none(), "nothing is saved yet");

    keyring.save("db-1", &typed("correct horse battery staple")).unwrap();
    keyring.save("db-2", &typed("a second database's passphrase")).unwrap();
    assert_eq!(keyring.load("db-1").unwrap().as_str(), "correct horse battery staple");
    assert_eq!(keyring.load("db-2").unwrap().as_str(), "a second database's passphrase");
    assert_eq!(portal.asked(), 1, "the secret is asked for once in a run");

    // A save replaces the one before it.
    keyring.save("db-1", &typed("a changed passphrase")).unwrap();
    assert_eq!(keyring.load("db-1").unwrap().as_str(), "a changed passphrase");

    // It is a file in the app's own folder, and the passphrase is not in it.
    let bytes = fs::read(&file).expect("the keyring file is written");
    assert!(!bytes.is_empty());
    assert!(
        !String::from_utf8_lossy(&bytes).contains("correct horse"),
        "the file holds no passphrase in the clear"
    );

    // A relaunch: a new store over the same file and the same secret.
    let relaunched = Keyring::portal(file.clone(), FixedPortal::with(SECRET));
    assert_eq!(relaunched.load("db-1").unwrap().as_str(), "a changed passphrase");

    // Forgetting one leaves the other, and forgetting nothing is fine.
    relaunched.forget("db-1").unwrap();
    assert!(relaunched.load("db-1").is_none());
    relaunched.forget("db-1").unwrap();
    assert_eq!(relaunched.load("db-2").unwrap().as_str(), "a second database's passphrase");
}

/// 011 FR-006: the entries carry the service and user names of every other
/// install, as the item's attributes.
#[test]
fn entries_keep_the_service_and_user_names() {
    let dir = TempDir::new().unwrap();
    let file = keyring_file(&dir);
    Keyring::portal(file.clone(), FixedPortal::with(SECRET))
        .save("0123abcd", &typed("remembered"))
        .unwrap();

    let found = zbus::block_on(async {
        let keyring =
            oo7::file::UnlockedKeyring::load(&file, oo7::Secret::blob(SECRET)).await.unwrap();
        let attributes = HashMap::from([
            ("service".to_owned(), SERVICE.to_owned()),
            ("username".to_owned(), "passphrase:0123abcd".to_owned()),
        ]);
        let item = keyring.lookup_item(&attributes).await.unwrap().expect("the item");
        (item.as_unlocked().secret().as_bytes().to_vec(), keyring.n_items().await)
    });
    assert_eq!(found, (b"remembered".to_vec(), 1));
}

/// 011 FR-006: a file written under another secret (another app id, or a keyring
/// the user reset) is not readable and not overwritten: nothing is read and
/// saving shows as unavailable (003 FR-019's error), with the file as it was.
#[test]
fn a_file_under_another_secret_is_left_alone() {
    let dir = TempDir::new().unwrap();
    let file = keyring_file(&dir);
    Keyring::portal(file.clone(), FixedPortal::with(SECRET)).save("db-1", &typed("kept")).unwrap();
    let before = fs::read(&file).unwrap();

    let other = Keyring::portal(file.clone(), FixedPortal::with(OTHER_SECRET));
    assert!(other.is_available(), "the portal answers; the file is the trouble");
    assert!(other.load("db-1").is_none());
    let err = other.save("db-1", &typed("overwritten")).unwrap_err();
    assert_eq!(err.code, "KEYRING_UNAVAILABLE");
    assert_eq!(fs::read(&file).unwrap(), before);
}

/// An empty secret from the portal is refused rather than used as a key.
#[test]
fn an_empty_secret_is_refused() {
    let dir = TempDir::new().unwrap();
    let file = keyring_file(&dir);
    let keyring = Keyring::portal(file.clone(), FixedPortal::with(b""));
    assert_eq!(keyring.save("db-1", &typed("x")).unwrap_err().code, "KEYRING_UNAVAILABLE");
    assert!(!file.exists());
}

// ------------------------------------------------------------------ stand-ins

/// The desktop portal of a desktop with a Secret backend. `RetrieveSecret` is
/// counted: the availability check must never call it.
struct SecretInterface {
    retrieved: Arc<AtomicUsize>,
}

#[zbus::interface(name = "org.freedesktop.portal.Secret")]
impl SecretInterface {
    // The portals' property is `version`, lower case, unlike zbus's default.
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }

    async fn retrieve_secret(
        &self,
        _fd: OwnedFd,
        _options: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<zbus::zvariant::OwnedObjectPath> {
        self.retrieved.fetch_add(1, Ordering::SeqCst);
        Err(zbus::fdo::Error::Failed("the availability check must not call this".into()))
    }
}

/// A desktop portal with another backend only, as one whose desktop has no
/// Secret backend: the service has an owner, the Secret interface is not there.
struct OpenUriInterface;

#[zbus::interface(name = "org.freedesktop.portal.OpenURI")]
impl OpenUriInterface {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        5
    }
}

/// A Secret Service (gnome-keyring, KWallet) and nothing else, as on a
/// `.deb` install's desktop: it must never be used by the Flatpak.
struct SecretServiceInterface {
    called: Arc<AtomicUsize>,
}

#[zbus::interface(name = "org.freedesktop.Secret.Service")]
impl SecretServiceInterface {
    async fn open_session(&self, _algorithm: String, _input: zbus::zvariant::Value<'_>) -> u32 {
        self.called.fetch_add(1, Ordering::SeqCst);
        0
    }

    #[zbus(property)]
    fn collections(&self) -> Vec<zbus::zvariant::OwnedObjectPath> {
        self.called.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
}

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

/// The portal on the bus's connection, as the app's availability check sees it.
struct BusPortal {
    connection: zbus::Connection,
    asked: AtomicUsize,
}

impl BusPortal {
    fn on(bus: &Bus) -> Arc<Self> {
        Arc::new(Self { connection: bus.connect(), asked: AtomicUsize::new(0) })
    }
}

impl SecretPortal for BusPortal {
    fn is_available(&self) -> bool {
        secret_portal_available(&self.connection)
    }

    fn retrieve_secret(&self) -> Result<Vec<u8>, String> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(SECRET.to_vec())
    }
}

// ------------------------------------------------------------------ availability

/// 011 FR-006: with a Secret portal, saving is available, and finding out never
/// calls `RetrieveSecret`.
#[test]
fn available_when_the_portal_has_a_secret_interface() {
    let bus = Bus::start();
    let retrieved = Arc::new(AtomicUsize::new(0));
    let _portal =
        bus.serve(PORTAL, PORTAL_PATH, SecretInterface { retrieved: Arc::clone(&retrieved) });

    assert!(secret_portal_available(&bus.connect()));

    let dir = TempDir::new().unwrap();
    let portal = BusPortal::on(&bus);
    let keyring = Keyring::portal(keyring_file(&dir), portal.clone());
    assert!(keyring.is_available());
    assert_eq!(retrieved.load(Ordering::SeqCst), 0, "RetrieveSecret was called to probe");
    assert_eq!(portal.asked.load(Ordering::SeqCst), 0);
}

/// 011 US2-5: no portal at all (nothing owns the name, and the bus can't start
/// it) shows saving as unavailable, and nothing is written.
#[test]
fn unavailable_with_no_portal_owner() {
    let bus = Bus::start();
    assert!(!secret_portal_available(&bus.connect()));

    let dir = TempDir::new().unwrap();
    let file = keyring_file(&dir);
    let portal = BusPortal::on(&bus);
    let keyring = Keyring::portal(file.clone(), portal.clone());
    assert!(!keyring.is_available());
    assert_eq!(keyring.save("db-1", &typed("x")).unwrap_err().code, "KEYRING_UNAVAILABLE");
    assert!(keyring.load("db-1").is_none());
    assert_eq!(keyring.forget("db-1").unwrap_err().code, "KEYRING_UNAVAILABLE");
    assert_eq!(portal.asked.load(Ordering::SeqCst), 0);
    assert!(!file.exists());
}

/// 011 US2-5: a portal whose desktop has no Secret backend (the service has an
/// owner, the Secret interface isn't exported) shows as unavailable.
#[test]
fn unavailable_when_the_portal_has_no_secret_interface() {
    let bus = Bus::start();
    let _portal = bus.serve(PORTAL, PORTAL_PATH, OpenUriInterface);
    assert!(!secret_portal_available(&bus.connect()));

    let dir = TempDir::new().unwrap();
    let keyring = Keyring::portal(keyring_file(&dir), BusPortal::on(&bus));
    assert!(!keyring.is_available());
    assert_eq!(keyring.save("db-1", &typed("x")).unwrap_err().code, "KEYRING_UNAVAILABLE");
}

/// 011 FR-006: a Secret Service on the bus is not a fallback. With one running
/// and no portal, saving is unavailable and the Secret Service is never called.
#[test]
fn unavailable_with_a_secret_service_and_no_portal() {
    let bus = Bus::start();
    let called = Arc::new(AtomicUsize::new(0));
    let _service = bus.serve(
        "org.freedesktop.secrets",
        "/org/freedesktop/secrets",
        SecretServiceInterface { called: Arc::clone(&called) },
    );

    let dir = TempDir::new().unwrap();
    let file = keyring_file(&dir);
    let keyring = Keyring::portal(file.clone(), BusPortal::on(&bus));
    assert!(!keyring.is_available());
    assert_eq!(keyring.save("db-1", &typed("x")).unwrap_err().code, "KEYRING_UNAVAILABLE");
    assert!(keyring.load("db-1").is_none());
    assert_eq!(called.load(Ordering::SeqCst), 0, "the Secret Service was called");
    assert!(!file.exists());
}

/// The portal service is usually started on first use. One the bus lists as
/// startable is asked, so a session whose portal hasn't run yet isn't told it
/// has none for the whole run; here it fails to start (`/bin/false`) and the
/// answer is unavailable, not a hang.
#[test]
fn a_portal_the_bus_cannot_start_is_unavailable() {
    let bus = Bus::start_with_services(&[(PORTAL, "/bin/false")]);
    let connection = bus.connect();
    let startable = zbus::block_on(async {
        zbus::fdo::DBusProxy::new(&connection).await?.list_activatable_names().await
    })
    .expect("listing the startable services");
    assert!(startable.iter().any(|name| name.as_str() == PORTAL), "the stand-in is startable");
    assert!(!secret_portal_available(&connection));
}
