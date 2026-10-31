//! Saved passphrases inside a Flatpak, through the Secret portal (011 FR-006;
//! 011 research.md §7; 011 data-model.md "Saved passphrases inside a Flatpak").
//!
//! The portal (`org.freedesktop.portal.Secret`) hands the app one secret of its
//! own, found from the caller's `.flatpak-info`. The entries are kept in oo7's
//! file keyring, encrypted with that secret, at
//! `$XDG_DATA_HOME/keyrings/default.keyring` (the Flatpak's own storage). So
//! HoploDex reaches its own saved passphrases and no other item of the user's
//! keyring, and the Flatpak asks for no bus name of the Secret Service.
//!
//! Only the portal path of oo7 is used: never `oo7::Keyring::new()`, which falls
//! back to the Secret Service when the portal is missing (011 FR-006 forbids any
//! fallback). Nothing in this file names `org.freedesktop.secrets`.
//!
//! [`Store`] is a keyring-core [`CredentialStoreApi`], so `services::keyring`
//! keeps one `Entry` path with the same service and user names as on every other
//! install. Where the secret comes from is a [`SecretPortal`], a parameter, so a
//! test supplies a fixed one.

use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use keyring_core::api::{CredentialApi, CredentialStoreApi};
use keyring_core::{Credential, CredentialPersistence, Entry, Error, Result};
use oo7::Secret;
use oo7::file::UnlockedKeyring;

const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SECRET_INTERFACE: &str = "org.freedesktop.portal.Secret";

/// The attribute names the entries are found by: the ones the `keyring`
/// crate's Secret Service store uses.
const SERVICE_ATTRIBUTE: &str = "service";
const USER_ATTRIBUTE: &str = "username";

/// What the desktop offers: whether there is a Secret portal, and the app's
/// secret from it.
pub trait SecretPortal: Send + Sync {
    /// Whether the desktop has a Secret portal. Never asks for the secret:
    /// the first `RetrieveSecret` makes the app's key and may prompt.
    fn is_available(&self) -> bool;

    /// The app's secret (`RetrieveSecret`), asked for once per run.
    fn retrieve_secret(&self) -> std::result::Result<Vec<u8>, String>;
}

/// The desktop's portal, on the session bus.
pub struct DesktopPortal;

impl SecretPortal for DesktopPortal {
    fn is_available(&self) -> bool {
        match zbus::block_on(zbus::Connection::session()) {
            Ok(connection) => secret_portal_available(&connection),
            Err(err) => {
                log::warn!("the session bus is not available, so no Secret portal: {err}");
                false
            }
        }
    }

    fn retrieve_secret(&self) -> std::result::Result<Vec<u8>, String> {
        zbus::block_on(oo7::ashpd::desktop::secret::retrieve())
            .map_err(|err| format!("the Secret portal gave no secret: {err}"))
    }
}

/// Whether the bus `connection` is on has a Secret portal: the portal service
/// has an owner (or can be started), and its `org.freedesktop.portal.Secret`
/// interface reads a `version`. xdg-desktop-portal exports that interface only
/// when a backend implements it. ashpd's `Secret::new()` answers `Ok` when the
/// portal service is missing, so the owner is checked first, and
/// `RetrieveSecret` is never called.
pub fn secret_portal_available(connection: &zbus::Connection) -> bool {
    match zbus::block_on(read_secret_portal_version(connection)) {
        Ok(version) => {
            log::info!("the Secret portal is available (version {version})");
            true
        }
        Err(reason) => {
            log::warn!("no Secret portal: {reason}");
            false
        }
    }
}

async fn read_secret_portal_version(
    connection: &zbus::Connection,
) -> std::result::Result<u32, String> {
    use zbus::names::BusName;

    let describe = |what: &str, err: &dyn std::fmt::Display| format!("{what}: {err}");
    let bus = zbus::fdo::DBusProxy::new(connection)
        .await
        .map_err(|err| describe("the bus daemon", &err))?;
    let name = BusName::try_from(PORTAL_NAME).map_err(|err| describe("the portal name", &err))?;
    let owned = bus
        .name_has_owner(name.clone())
        .await
        .map_err(|err| describe("asking who owns the portal", &err))?;
    if !owned {
        // xdg-desktop-portal is usually started on first use. Reading the
        // property below starts it; without that, a session whose portal has
        // not been called yet would be told it has none, for the whole run.
        let activatable = bus
            .list_activatable_names()
            .await
            .map_err(|err| describe("listing the services the bus can start", &err))?;
        if !activatable.iter().any(|candidate| candidate.as_str() == PORTAL_NAME) {
            return Err("nothing owns org.freedesktop.portal.Desktop".to_owned());
        }
    }
    let proxy = zbus::proxy::Builder::<zbus::Proxy>::new(connection)
        .destination(PORTAL_NAME)
        .and_then(|builder| builder.path(PORTAL_PATH))
        .and_then(|builder| builder.interface(SECRET_INTERFACE))
        .map_err(|err| describe("the Secret portal's address", &err))?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .map_err(|err| describe("the Secret portal", &err))?;
    proxy
        .get_property::<u32>("version")
        .await
        .map_err(|err| describe("the Secret portal's version", &err))
}

/// The keyring file inside the Flatpak's own data folder:
/// `$XDG_DATA_HOME/keyrings/default.keyring`, `XDG_DATA_HOME` defaulting to
/// `$HOME/.local/share`. `None` when neither is set to an absolute path.
pub fn default_keyring_file() -> Option<PathBuf> {
    let absolute =
        |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|path| path.is_absolute());
    let data_home = absolute("XDG_DATA_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".local").join("share")))?;
    Some(data_home.join("keyrings").join("default.keyring"))
}

/// The portal-backed credential store. Cheap to clone: the loaded keyring is
/// shared.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

struct Inner {
    file: PathBuf,
    portal: Arc<dyn SecretPortal>,
    /// The file keyring, loaded with the portal's secret the first time an
    /// entry needs it, and kept for the run.
    keyring: Mutex<Option<Arc<UnlockedKeyring>>>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").field("file", &self.inner.file).finish_non_exhaustive()
    }
}

impl Store {
    /// A store over the keyring file `file`, asking `portal` for the secret.
    pub fn new(file: PathBuf, portal: Arc<dyn SecretPortal>) -> Self {
        Self { inner: Arc::new(Inner { file, portal, keyring: Mutex::new(None) }) }
    }

    /// Whether the portal is there (003 FR-019). Never asks for the secret.
    pub fn is_available(&self) -> bool {
        self.inner.portal.is_available()
    }

    pub fn file(&self) -> &Path {
        &self.inner.file
    }
}

impl Inner {
    /// The unlocked keyring, loaded on first use. A failure is not kept: the
    /// next call asks again.
    fn keyring(&self) -> Result<Arc<UnlockedKeyring>> {
        let mut slot = self.keyring.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(keyring) = slot.as_ref() {
            return Ok(keyring.clone());
        }
        let secret = self
            .portal
            .retrieve_secret()
            .map_err(|reason| Error::NoStorageAccess(reason.into()))?;
        if secret.is_empty() {
            return Err(Error::NoStorageAccess("the Secret portal gave an empty secret".into()));
        }
        let keyring = zbus::block_on(UnlockedKeyring::load(&self.file, Secret::blob(secret)))
            .map_err(platform_failure)?;
        let keyring = Arc::new(keyring);
        *slot = Some(keyring.clone());
        Ok(keyring)
    }
}

fn platform_failure(err: oo7::file::Error) -> Error {
    Error::PlatformFailure(err.to_string().into())
}

impl CredentialStoreApi for Store {
    fn vendor(&self) -> String {
        "HoploDex Secret portal store (oo7 file keyring)".to_owned()
    }

    fn id(&self) -> String {
        format!("hoplodex-secret-portal:{}", self.inner.file.display())
    }

    fn build(
        &self,
        service: &str,
        user: &str,
        modifiers: Option<&HashMap<&str, &str>>,
    ) -> Result<Entry> {
        if modifiers.is_some_and(|modifiers| !modifiers.is_empty()) {
            return Err(Error::NotSupportedByStore(
                "the Secret portal store takes no modifiers".to_owned(),
            ));
        }
        if service.is_empty() || user.is_empty() {
            return Err(Error::Invalid("service or user".to_owned(), "is empty".to_owned()));
        }
        Ok(Entry::new_with_credential(Arc::new(Cred {
            inner: self.inner.clone(),
            service: service.to_owned(),
            user: user.to_owned(),
        })))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::UntilDelete
    }
}

/// One entry: the item whose attributes are its service and user.
struct Cred {
    inner: Arc<Inner>,
    service: String,
    user: String,
}

impl std::fmt::Debug for Cred {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cred")
            .field("service", &self.service)
            .field("user", &self.user)
            .finish_non_exhaustive()
    }
}

impl Cred {
    fn attributes(&self) -> HashMap<String, String> {
        HashMap::from([
            (SERVICE_ATTRIBUTE.to_owned(), self.service.clone()),
            (USER_ATTRIBUTE.to_owned(), self.user.clone()),
        ])
    }

    fn store(&self, secret: Secret) -> Result<()> {
        let keyring = self.inner.keyring()?;
        let label = format!("{} ({})", self.user, self.service);
        zbus::block_on(keyring.create_item(&label, &self.attributes(), secret, true))
            .map(drop)
            .map_err(platform_failure)
    }
}

impl CredentialApi for Cred {
    fn set_password(&self, password: &str) -> Result<()> {
        self.store(Secret::text(password))
    }

    fn set_secret(&self, secret: &[u8]) -> Result<()> {
        self.store(Secret::blob(secret))
    }

    fn get_secret(&self) -> Result<Vec<u8>> {
        let keyring = self.inner.keyring()?;
        let item = zbus::block_on(keyring.lookup_item(&self.attributes()))
            .map_err(platform_failure)?
            .ok_or(Error::NoEntry)?;
        Ok(item.as_unlocked().secret().as_bytes().to_vec())
    }

    fn delete_credential(&self) -> Result<()> {
        let keyring = self.inner.keyring()?;
        let attributes = self.attributes();
        zbus::block_on(keyring.lookup_item(&attributes))
            .map_err(platform_failure)?
            .ok_or(Error::NoEntry)?;
        zbus::block_on(keyring.delete(&attributes)).map_err(platform_failure)
    }

    fn get_credential(&self) -> Result<Option<Arc<Credential>>> {
        self.get_secret().map(|_| None)
    }

    fn get_specifiers(&self) -> Option<(String, String)> {
        Some((self.service.clone(), self.user.clone()))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
