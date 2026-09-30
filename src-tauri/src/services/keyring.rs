//! Passphrases saved in this computer's keyring, one per database, only when
//! the user asks (FR-017–FR-019, research.md §10; data-model.md "Keyring:
//! saved passphrase"). Each is the entry service `com.hoplodex.app`, user
//! `passphrase:<database_id>`, keyed by the database's id rather than its
//! path, so moving the file keeps it and a backup opened directly (which
//! gets a new id) does not inherit it.
//!
//! The pre-feature `sqlcipher-key` entry, which keys the developer's real
//! database, is never read, written or deleted: every user name used here
//! starts with `passphrase:`.

use std::sync::OnceLock;

use crate::commands::CommandError;
use crate::services::passphrase::Passphrase;

/// Fixed on purpose, not derived from the bundle identifier (now
/// `io.github.exodious.HoploDex`): changing it would orphan every saved
/// passphrase.
const SERVICE: &str = "com.hoplodex.app";
const USER_PREFIX: &str = "passphrase:";
/// A user no database has (ids are hex digits), read to find out whether the
/// keyring answers at all.
const PROBE_USER: &str = "passphrase:availability-check";

#[cfg(not(feature = "mock-keyring"))]
type Entry = ::keyring::Entry;
#[cfg(feature = "mock-keyring")]
type Entry = keyring_core::Entry;
type KeyringError = ::keyring::Error;

/// This computer's keyring as HoploDex uses it. Kept by
/// [`MachineSettings`](crate::services::machine_settings::MachineSettings),
/// with the recent list whose `passphraseSaved` flags it backs.
pub struct Keyring {
    backend: Backend,
    /// Whether the keyring answers, probed the first time it is needed and
    /// then kept for the session (FR-019).
    available: OnceLock<bool>,
}

enum Backend {
    /// No keyring: tests and tools that must never reach the OS keyring,
    /// and E2E runs that play a computer without one.
    Off,
    /// The OS keyring (Secret Service, Keychain, Credential Manager), or,
    /// under `mock-keyring`, keyring-core's in-memory store.
    System {
        /// E2E only: a file the in-memory store is loaded from and written
        /// back to, so a saved passphrase outlives a relaunch.
        #[cfg(feature = "mock-keyring")]
        mirror: Option<std::path::PathBuf>,
    },
}

impl Keyring {
    /// A keyring that is never touched and reports itself unavailable.
    pub fn off() -> Self {
        Self { backend: Backend::Off, available: OnceLock::new() }
    }

    /// The OS keyring.
    #[cfg(not(feature = "mock-keyring"))]
    pub fn system() -> Self {
        Self { backend: Backend::System {}, available: OnceLock::new() }
    }

    /// The in-memory keyring of an E2E or test build, configured by
    /// `HOPLODEX_E2E_KEYRING=unavailable` (a computer with no keyring) and
    /// `HOPLODEX_E2E_KEYRING_FILE` (a file that keeps it across launches).
    #[cfg(feature = "mock-keyring")]
    pub fn system() -> Self {
        Self::mock(
            std::env::var("HOPLODEX_E2E_KEYRING").ok().as_deref(),
            std::env::var_os("HOPLODEX_E2E_KEYRING_FILE").map(Into::into),
        )
    }

    /// [`system`](Self::system) with its two settings given, so tests set
    /// them without touching the process environment.
    #[cfg(feature = "mock-keyring")]
    pub fn mock(setting: Option<&str>, mirror: Option<std::path::PathBuf>) -> Self {
        if setting == Some("unavailable") {
            return Self::off();
        }
        mock::install_store();
        if let Some(file) = &mirror {
            mock::read_mirror(file);
        }
        Self { backend: Backend::System { mirror }, available: OnceLock::new() }
    }

    /// Whether passphrases can be saved on this computer (FR-019). A
    /// failure's reason is logged, never shown.
    pub fn is_available(&self) -> bool {
        *self.available.get_or_init(|| match &self.backend {
            Backend::Off => false,
            Backend::System { .. } => match entry(PROBE_USER).and_then(|e| e.get_password()) {
                Ok(_) | Err(KeyringError::NoEntry) => true,
                Err(err) => {
                    log::warn!("the keyring is not available: {err}");
                    false
                }
            },
        })
    }

    /// Saves `passphrase` for the database `database_id`, replacing any
    /// saved before. `KEYRING_UNAVAILABLE` when it can't be.
    pub fn save(&self, database_id: &str, passphrase: &Passphrase) -> Result<(), CommandError> {
        if !self.is_available() {
            return Err(CommandError::keyring_unavailable());
        }
        let user = user_for(database_id);
        entry(&user).and_then(|e| e.set_password(passphrase.as_str())).map_err(|err| {
            log::error!("could not save a passphrase in the keyring: {err}");
            CommandError::keyring_unavailable()
        })?;
        self.mirror_changed();
        Ok(())
    }

    /// The passphrase saved for `database_id`, if there is one and the
    /// keyring can be read.
    pub fn load(&self, database_id: &str) -> Option<Passphrase> {
        if !self.is_available() {
            return None;
        }
        match entry(&user_for(database_id)).and_then(|e| e.get_password()) {
            Ok(saved) => Some(Passphrase::from_input(saved)),
            Err(KeyringError::NoEntry) => None,
            Err(err) => {
                log::warn!("could not read a saved passphrase from the keyring: {err}");
                None
            }
        }
    }

    /// Deletes the passphrase saved for `database_id`. Having none is fine;
    /// `KEYRING_UNAVAILABLE` when one may still be there.
    pub fn forget(&self, database_id: &str) -> Result<(), CommandError> {
        if !self.is_available() {
            return Err(CommandError::keyring_unavailable());
        }
        match entry(&user_for(database_id)).and_then(|e| e.delete_credential()) {
            Ok(()) | Err(KeyringError::NoEntry) => {
                self.mirror_changed();
                Ok(())
            }
            Err(err) => {
                log::error!("could not delete a saved passphrase from the keyring: {err}");
                Err(CommandError::keyring_unavailable())
            }
        }
    }

    #[cfg(not(feature = "mock-keyring"))]
    fn mirror_changed(&self) {}

    #[cfg(feature = "mock-keyring")]
    fn mirror_changed(&self) {
        if let Backend::System { mirror: Some(file) } = &self.backend {
            mock::write_mirror(file);
        }
    }
}

fn user_for(database_id: &str) -> String {
    format!("{USER_PREFIX}{database_id}")
}

#[cfg(not(feature = "mock-keyring"))]
fn entry(user: &str) -> Result<Entry, KeyringError> {
    debug_assert!(user.starts_with(USER_PREFIX));
    Entry::new(SERVICE, user)
}

#[cfg(feature = "mock-keyring")]
fn entry(user: &str) -> Result<Entry, KeyringError> {
    debug_assert!(user.starts_with(USER_PREFIX));
    // A test may have put in a store of its own; otherwise make one.
    mock::install_store();
    Entry::new(SERVICE, user)
}

/// The in-memory store of `mock-keyring` builds, and the file E2E runs keep
/// it in between launches. The file holds passphrases in the clear: it is
/// only ever in an E2E run's scratch directory.
#[cfg(feature = "mock-keyring")]
mod mock {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::sync::Once;

    use super::{Entry, SERVICE, USER_PREFIX};

    static INSTALLED: Once = Once::new();

    /// Makes the in-memory store the default one, unless a test put one in
    /// first.
    pub(super) fn install_store() {
        INSTALLED.call_once(|| {
            if keyring_core::get_default_store().is_none() {
                keyring_core::set_default_store(
                    keyring_core::mock::Store::new().expect("mock keyring store"),
                );
            }
        });
    }

    /// Loads the passphrases a previous launch wrote to `file`.
    pub(super) fn read_mirror(file: &Path) {
        let Ok(bytes) = fs::read(file) else { return };
        let saved: BTreeMap<String, String> = serde_json::from_slice(&bytes).unwrap_or_default();
        for (user, passphrase) in saved {
            if user.starts_with(USER_PREFIX)
                && let Ok(entry) = Entry::new(SERVICE, &user)
            {
                let _ = entry.set_password(&passphrase);
            }
        }
    }

    /// Writes every saved passphrase to `file`.
    pub(super) fn write_mirror(file: &Path) {
        let spec = std::collections::HashMap::from([("service", SERVICE)]);
        let saved: BTreeMap<String, String> = Entry::search(&spec)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                // Anything else under the service, such as `sqlcipher-key`,
                // is not read.
                let (_, user) =
                    entry.get_specifiers().filter(|(_, u)| u.starts_with(USER_PREFIX))?;
                Some((user, entry.get_password().ok()?))
            })
            .collect();
        match serde_json::to_vec(&saved) {
            Ok(json) => {
                if let Err(err) = fs::write(file, json) {
                    log::warn!("could not write the E2E keyring file: {err}");
                }
            }
            Err(err) => log::warn!("could not write the E2E keyring file: {err}"),
        }
    }
}
