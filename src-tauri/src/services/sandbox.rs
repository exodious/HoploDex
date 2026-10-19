//! What this run of HoploDex may reach on the filesystem (011 research.md §5,
//! 011 data-model.md "Reach" and "Documents-portal paths").
//!
//! Inside a Flatpak the app sees only the folders its manifest and the user's
//! overrides grant. [`Reach`] is read once at startup from `/.flatpak-info` and
//! judges a path before anything is written, so a location outside it is
//! refused with `LOCATION_OUT_OF_REACH` (and the fix) instead of failing part
//! way with an I/O error that looks like a missing folder. Outside a Flatpak
//! it is [`Reach::Unconfined`] and every path is within reach, so a `.deb`,
//! `.rpm`, AppImage, macOS or Windows install changes in no way.
//!
//! Nothing here writes: no probe file, no scratch file. Everything but the
//! Linux-only Documents-portal lookup ([`PortalHostPaths`]) is plain code, so
//! it is tested on every OS.

use std::collections::HashMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::commands::CommandError;

/// Where Flatpak describes the sandbox to the app inside it.
pub const FLATPAK_INFO: &str = "/.flatpak-info";

/// A folder the sandbox gives this copy of HoploDex, and the whole of what is
/// below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub root: PathBuf,
    pub writable: bool,
}

/// What an operation needs of a location (011 data-model.md "Need").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// A database file: its folder must be within a writable grant. The path
    /// is the database file's.
    DatabaseFolder,
    /// A folder to write into once, now. The path is the folder.
    WriteFolder,
    /// A folder backups are written to, now and at every later close. The path
    /// is the folder. Like [`Need::WriteFolder`], except that a Documents-portal
    /// path outside reach is refused (011 T097): the portal's folder is the
    /// chooser's answer to one question, not a place to keep writing to.
    BackupFolder,
    /// A file to read. The path is the file.
    ReadFile,
}

/// Turns a Documents-portal path (`/run/user/<uid>/doc/<id>/<name>`) into the
/// path the same file has on the host.
pub trait HostPaths: Send + Sync {
    fn host_path(&self, portal_path: &Path) -> Option<PathBuf>;
}

/// A [`HostPaths`] that asks, per document, where the document is on the host:
/// `lookup(document id, the document's entry under doc/<id>/)`. It does what the
/// Documents portal's answer needs done, whoever gives the answer: the
/// document's host path stands for `doc/<id>/<name>` (the file, or a picked
/// folder), so the rest of the path below `<name>` is joined onto it (011 T094).
/// The app's lookup is [`PortalHostPaths`]; a test's is a map by document id.
pub struct ByDocument<F>(pub F);

impl<F> HostPaths for ByDocument<F>
where
    F: Fn(&str, &Path) -> Option<PathBuf> + Send + Sync,
{
    fn host_path(&self, portal_path: &Path) -> Option<PathBuf> {
        let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let portal_path = absolute(portal_path);
        let document = split_portal(&portal_path, runtime_dir.as_deref())?;
        let host = (self.0)(&document.id, &document.entry)?;
        Some(normalize(&host.join(document.below)))
    }
}

/// No portal to ask: every portal path stays unresolved.
struct NoHostPaths;

impl HostPaths for NoHostPaths {
    fn host_path(&self, _portal_path: &Path) -> Option<PathBuf> {
        None
    }
}

/// The user's folders from `user-dirs.dirs` (`XDG_DOCUMENTS_DIR="$HOME/Documents"`
/// and the others), which `xdg-documents` and its siblings in a Flatpak's
/// grants name.
#[derive(Debug, Clone, Default)]
pub struct UserDirs {
    /// `DOCUMENTS` for `XDG_DOCUMENTS_DIR`, and so on.
    dirs: HashMap<String, PathBuf>,
}

impl UserDirs {
    /// Reads `user-dirs.dirs` in `config_home` (the caller passes
    /// `$XDG_CONFIG_HOME`, never a hard-coded folder). A missing or unreadable
    /// file gives no folders, so each `xdg-*` grant is then not counted.
    pub fn read(config_home: &Path, home: &Path) -> Self {
        match std::fs::read_to_string(config_home.join("user-dirs.dirs")) {
            Ok(text) => Self::parse(&text, home),
            Err(_) => Self::default(),
        }
    }

    /// The file is shell-sourced: `NAME="value"` lines, `#` comments, and a
    /// value that may start with `$HOME` or `${HOME}`. A value that is not an
    /// absolute path after that is left out.
    pub fn parse(text: &str, home: &Path) -> Self {
        let mut dirs = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((name, value)) = line.split_once('=') else { continue };
            let Some(key) = name
                .trim()
                .strip_prefix("XDG_")
                .and_then(|rest| rest.strip_suffix("_DIR"))
                .filter(|key| !key.is_empty())
            else {
                continue;
            };
            let value = unquote(value.trim());
            let expanded = if let Some(rest) = value.strip_prefix("${HOME}") {
                home.join(rest.trim_start_matches('/'))
            } else if let Some(rest) = value.strip_prefix("$HOME") {
                home.join(rest.trim_start_matches('/'))
            } else {
                PathBuf::from(&value)
            };
            if expanded.is_absolute() {
                dirs.insert(key.to_owned(), normalize(&expanded));
            }
        }
        Self { dirs }
    }

    /// The folder `xdg-<name>` stands for: `documents`, `download`,
    /// `public-share` (key `PUBLICSHARE`).
    fn folder(&self, name: &str) -> Option<&Path> {
        let key: String = name.chars().filter(|c| *c != '-').collect::<String>().to_uppercase();
        self.dirs.get(&key).map(PathBuf::as_path)
    }
}

/// A value without its quotes, and the backslash escapes a quoted shell string
/// has.
fn unquote(value: &str) -> String {
    let inner = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|rest| rest.strip_suffix('\'')))
        .unwrap_or(value);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// What this run may reach.
#[derive(Clone)]
pub enum Reach {
    /// Not in a Flatpak: every path is within reach and nothing is checked.
    Unconfined,
    /// In a Flatpak, with the grants `/.flatpak-info` lists.
    Flatpak {
        grants: Vec<Grant>,
        /// Roots an override removed (`!<entry>`). Where a removed root and a
        /// grant are equally close to a path, the removal wins, so a mistake
        /// narrows reach and never widens it.
        removed: Vec<PathBuf>,
        home: PathBuf,
        host_paths: Arc<dyn HostPaths>,
    },
}

impl fmt::Debug for Reach {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unconfined => f.write_str("Unconfined"),
            Self::Flatpak { grants, removed, home, .. } => f
                .debug_struct("Flatpak")
                .field("grants", grants)
                .field("removed", removed)
                .field("home", home)
                .finish_non_exhaustive(),
        }
    }
}

/// How far a path is within reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    None,
    ReadOnly,
    Writable,
}

/// One entry of `filesystems=`.
enum Entry {
    Root {
        root: PathBuf,
        writable: bool,
    },
    /// Understood and deliberately not counted (`host-os`, `host-etc`).
    Skipped,
    /// Not understood: not counted, and logged.
    Unknown,
}

impl Reach {
    /// A Flatpak with these grants.
    pub fn flatpak(
        grants: Vec<Grant>,
        removed: Vec<PathBuf>,
        home: PathBuf,
        host_paths: Arc<dyn HostPaths>,
    ) -> Self {
        Self::Flatpak { grants, removed, home, host_paths }
    }

    /// Reads `info_path` (`/.flatpak-info`) once. No file there means this is
    /// not a Flatpak. A file that can't be read means it is one, with no grants
    /// (nothing within reach, and each operation refused with the fix, rather
    /// than failing part way).
    pub fn detect(
        info_path: &Path,
        home: &Path,
        user_dirs: &UserDirs,
        host_paths: Arc<dyn HostPaths>,
    ) -> Self {
        match std::fs::read_to_string(info_path) {
            Ok(text) => match Self::from_flatpak_info(&text, home, user_dirs) {
                Self::Flatpak { grants, removed, home, .. } => {
                    Self::Flatpak { grants, removed, home, host_paths }
                }
                unconfined => unconfined,
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::Unconfined,
            Err(err) => {
                log::warn!(
                    "{} can't be read, so nothing is within reach: {err}",
                    info_path.display()
                );
                Self::flatpak(Vec::new(), Vec::new(), home.to_owned(), host_paths)
            }
        }
    }

    /// The reach `/.flatpak-info`'s text describes: the `[Context]` group's
    /// `filesystems=` key (011 data-model.md "Reading `/.flatpak-info`"). Pure.
    /// Portal paths stay unresolved; [`Reach::detect`] gives the app's lookup.
    pub fn from_flatpak_info(text: &str, home: &Path, user_dirs: &UserDirs) -> Self {
        let mut grants = Vec::new();
        let mut removed = Vec::new();
        for raw in filesystems(text).unwrap_or_default().split(';') {
            let entry = raw.trim();
            if entry.is_empty() {
                continue;
            }
            let (removes, entry) = match entry.strip_prefix('!') {
                Some(rest) => (true, rest.trim()),
                None => (false, entry),
            };
            match parse_entry(entry, home, user_dirs) {
                Entry::Root { root, .. } if removes => removed.push(root),
                Entry::Root { root, writable } => grants.push(Grant { root, writable }),
                Entry::Skipped => {}
                Entry::Unknown => {
                    log::warn!(
                        "filesystem entry {raw:?} in {FLATPAK_INFO} is not understood, so it is not counted"
                    );
                }
            }
        }
        Self::flatpak(grants, removed, home.to_owned(), Arc::new(NoHostPaths))
    }

    pub fn is_flatpak(&self) -> bool {
        matches!(self, Self::Flatpak { .. })
    }

    /// The grants, empty when unconfined.
    pub fn grants(&self) -> &[Grant] {
        match self {
            Self::Unconfined => &[],
            Self::Flatpak { grants, .. } => grants,
        }
    }

    /// One line for the log at startup.
    pub fn describe(&self) -> String {
        match self {
            Self::Unconfined => "not running in a Flatpak: every location is within reach".into(),
            Self::Flatpak { grants, removed, .. } => {
                let list =
                    |paths: &mut dyn Iterator<Item = String>| paths.collect::<Vec<_>>().join(", ");
                let granted = list(&mut grants.iter().map(|grant| {
                    format!(
                        "{}{}",
                        grant.root.display(),
                        if grant.writable { "" } else { " (read-only)" }
                    )
                }));
                let taken = list(&mut removed.iter().map(|root| root.display().to_string()));
                format!(
                    "running in a Flatpak; reach: [{granted}]{}",
                    if taken.is_empty() { String::new() } else { format!(", removed: [{taken}]") }
                )
            }
        }
    }

    /// How far `path` is within a grant (compared after making it absolute and
    /// normalising `.` and `..`): the longest root that holds it decides.
    ///
    /// Links are followed in the grant roots and in the path's folders above
    /// them, so `/home/someone` is judged against a grant on `/var/home/someone`
    /// where `/home` links to `/var/home`; a link inside a grant is judged by
    /// its own path, as bwrap binds it (011 T112).
    fn access(&self, path: &Path) -> Access {
        let Self::Flatpak { grants, removed, .. } = self else { return Access::Writable };
        let path = absolute(path);
        let originals: Vec<&Path> = grants
            .iter()
            .map(|grant| grant.root.as_path())
            .chain(removed.iter().map(PathBuf::as_path))
            .collect();
        let resolved_grants: Vec<Grant> = grants
            .iter()
            .map(|grant| Grant { root: resolve(&grant.root), writable: grant.writable })
            .collect();
        let resolved_removed: Vec<PathBuf> = removed.iter().map(|root| resolve(root)).collect();
        let resolved_roots: Vec<&Path> = resolved_grants
            .iter()
            .map(|grant| grant.root.as_path())
            .chain(resolved_removed.iter().map(PathBuf::as_path))
            .collect();
        let path = locate(&path, &originals, &resolved_roots);
        let (grants, removed) = (&resolved_grants, &resolved_removed);
        let depth = |root: &Path| root.components().count();
        let mut best = None::<usize>;
        let mut access = Access::None;
        for grant in grants.iter().filter(|grant| path.starts_with(&grant.root)) {
            let d = depth(&grant.root);
            let this = if grant.writable { Access::Writable } else { Access::ReadOnly };
            match best {
                Some(b) if d < b => {}
                Some(b) if d == b => access = access.max_with(this),
                _ => {
                    best = Some(d);
                    access = this;
                }
            }
        }
        // A removal as close as the closest grant, or closer, takes it away.
        let closest_removal =
            removed.iter().filter(|root| path.starts_with(root)).map(|r| depth(r)).max();
        match (closest_removal, best) {
            (Some(r), Some(b)) if r >= b => Access::None,
            (Some(_), None) => Access::None,
            _ => access,
        }
    }

    /// Judges `path` for `need` and returns the path to use: the given one, or
    /// the host path a Documents-portal path stands for when that is within
    /// reach. Refuses with `LOCATION_OUT_OF_REACH` otherwise. Writes nothing
    /// and makes no scratch file (011 research.md §5).
    ///
    /// For `DatabaseFolder` the path is the database file, whose folder is what
    /// must be within a writable grant; for `WriteFolder` it is the folder.
    pub fn check(&self, path: &Path, need: Need) -> Result<PathBuf, CommandError> {
        let Self::Flatpak { host_paths, .. } = self else { return Ok(path.to_owned()) };
        let given = absolute(path);
        let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        if portal_document(&given, runtime_dir.as_deref()).is_some() {
            return self.check_portal(path, &given, host_paths.as_ref(), need);
        }
        match need {
            Need::DatabaseFolder | Need::WriteFolder | Need::BackupFolder => {
                let folder = needed_folder(&given, need);
                match self.access(&folder) {
                    Access::Writable => Ok(path.to_owned()),
                    _ => Err(self.refuse(path, &folder)),
                }
            }
            Need::ReadFile => {
                // Only a file a grant covers is within reach, whether or not
                // it exists in the sandbox (011 FR-007: a drop from outside
                // reach is always refused, even where the same path happens to
                // show up in the sandbox). A missing one inside a grant is
                // passed on so the caller says "not found" as it does today.
                if self.access(&given) != Access::None {
                    Ok(path.to_owned())
                } else {
                    Err(self.refuse(path, &needed_folder(&given, need)))
                }
            }
        }
    }

    fn check_portal(
        &self,
        path: &Path,
        portal_path: &Path,
        host_paths: &dyn HostPaths,
        need: Need,
    ) -> Result<PathBuf, CommandError> {
        // A refusal names the host path and the host folder when they are
        // known, so the override command it gives names a folder that helps
        // (011 T095); the portal's own folder helps nobody.
        if let Some(host) = host_paths.host_path(portal_path).map(|host| absolute(&host)) {
            let folder = needed_folder(&host, need);
            let probe = if need == Need::ReadFile { &host } else { &folder };
            let stays_a_portal_path = matches!(need, Need::WriteFolder | Need::ReadFile);
            match (self.access(probe), need) {
                (Access::Writable, _) | (Access::ReadOnly, Need::ReadFile) => return Ok(host),
                (Access::ReadOnly, _) => return Err(self.refuse(&host, &folder)),
                (Access::None, _) if !stays_a_portal_path => {
                    return Err(self.refuse(&host, &folder));
                }
                (Access::None, _) => {}
            }
        }
        // Outside every grant, or not resolved: writing or reading inside the
        // portal's own folder works once, but a database's folder is not that
        // and a stored backup folder is not to be kept (011 T097).
        match need {
            Need::DatabaseFolder | Need::BackupFolder => {
                Err(self.refuse(path, &needed_folder(portal_path, need)))
            }
            Need::WriteFolder | Need::ReadFile => Ok(path.to_owned()),
        }
    }

    fn refuse(&self, path: &Path, folder: &Path) -> CommandError {
        let home = match self {
            Self::Unconfined => Path::new(""),
            Self::Flatpak { home, .. } => home.as_path(),
        };
        CommandError::out_of_reach(path, folder, home, self.grants())
    }
}

impl Access {
    fn max_with(self, other: Access) -> Access {
        if self == Access::Writable || other == Access::Writable {
            Access::Writable
        } else if self == Access::ReadOnly || other == Access::ReadOnly {
            Access::ReadOnly
        } else {
            Access::None
        }
    }
}

/// The folder whose reach `need` asks about for `path`.
fn needed_folder(path: &Path, need: Need) -> PathBuf {
    match need {
        Need::WriteFolder | Need::BackupFolder => path.to_owned(),
        Need::DatabaseFolder | Need::ReadFile => path.parent().unwrap_or(path).to_owned(),
    }
}

/// The `filesystems=` value of the `[Context]` group, a `;`-separated list.
fn filesystems(text: &str) -> Option<&str> {
    let mut in_context = false;
    let mut value = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_context = line == "[Context]";
        } else if in_context
            && let Some((key, rest)) = line.split_once('=')
            && key.trim() == "filesystems"
        {
            value = Some(rest.trim());
        }
    }
    value
}

/// One grant entry (011 data-model.md "Reading `/.flatpak-info`").
fn parse_entry(entry: &str, home: &Path, user_dirs: &UserDirs) -> Entry {
    let (spec, writable) = match entry.rsplit_once(':') {
        Some((spec, "ro")) => (spec, false),
        Some((spec, "rw" | "create")) => (spec, true),
        Some(_) => return Entry::Unknown,
        None => (entry, true),
    };
    let root = match spec {
        "home" | "~" => home.to_owned(),
        "host" => PathBuf::from("/"),
        "host-os" | "host-etc" => return Entry::Skipped,
        _ => {
            if let Some(rest) = spec.strip_prefix("~/") {
                home.join(rest)
            } else if spec.starts_with('/') {
                PathBuf::from(spec)
            } else if let Some(user_folder) = spec.strip_prefix("xdg-") {
                let (name, sub) = user_folder.split_once('/').unwrap_or((user_folder, ""));
                match user_dirs.folder(name) {
                    Some(folder) => folder.join(sub),
                    None => return Entry::Unknown,
                }
            } else {
                return Entry::Unknown;
            }
        }
    };
    Entry::Root { root: normalize(&root), writable }
}

/// `path` made absolute and with `.` and `..` taken out, without following
/// links (a link inside the home folder that points outside is judged by its
/// own path, as bwrap binds it).
fn absolute(path: &Path) -> PathBuf {
    normalize(&std::path::absolute(path).unwrap_or_else(|_| path.to_owned()))
}

/// `path` with its links followed, as far as it exists. Unix only: a Flatpak
/// runs only on Linux, and elsewhere nothing is followed.
fn canonical(path: &Path) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::fs::canonicalize(path).ok()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// `path` with the links in its existing part followed (itself included when it
/// exists) and the missing rest unchanged.
fn resolve(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if let Some(found) = canonical(ancestor)
            && let Ok(rest) = path.strip_prefix(ancestor)
        {
            return normalize(&found.join(rest));
        }
    }
    path.to_owned()
}

/// `path` as it is named under the resolved grant roots: its longest folder
/// that is not inside a grant or removed root (`originals`, as given, and
/// `resolved`) and whose real path is that of a root or above one has its links
/// followed, and the rest stays as it is, so a link inside a grant is judged by
/// its own path (011 T112). A path none of that applies to is returned as it
/// is.
fn locate(path: &Path, originals: &[&Path], resolved: &[&Path]) -> PathBuf {
    for ancestor in path.ancestors() {
        let inside_a_root = |root: &&Path| ancestor.starts_with(root) && ancestor != *root;
        if originals.iter().chain(resolved).any(inside_a_root) {
            continue;
        }
        if let Some(found) = canonical(ancestor)
            && resolved.iter().any(|root| root.starts_with(&found))
            && let Ok(rest) = path.strip_prefix(ancestor)
        {
            return normalize(&found.join(rest));
        }
    }
    path.to_owned()
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// A path under the Documents portal's folder, taken apart.
#[derive(Debug, PartialEq, Eq)]
struct PortalDocument {
    /// The document's id: `<id>` in `doc/<id>/<name>/…`.
    id: String,
    /// `doc/<id>/<name>`, the document itself: the file, or the picked folder.
    /// The portal's lookups (`GetHostPaths`, the host-path attribute) answer
    /// for this entry.
    entry: PathBuf,
    /// What lies below the entry (empty for a file document).
    below: PathBuf,
}

/// `path` taken apart when it is under the Documents portal's folder
/// (`$XDG_RUNTIME_DIR/doc/<id>/…` or `/run/user/<uid>/doc/<id>/…`). `path` is
/// absolute and normalised.
fn split_portal(path: &Path, runtime_dir: Option<&Path>) -> Option<PortalDocument> {
    let split_after = |doc_folder: &Path| {
        let rest = path.strip_prefix(doc_folder).ok()?;
        let mut parts = rest.components();
        let id = parts.next()?.as_os_str().to_str()?.to_owned();
        let mut entry = doc_folder.join(&id);
        if let Some(name) = parts.next() {
            entry.push(name);
        }
        Some(PortalDocument { id, entry, below: parts.as_path().to_owned() })
    };
    if let Some(found) =
        runtime_dir.filter(|dir| dir.is_absolute()).and_then(|dir| split_after(&dir.join("doc")))
    {
        return Some(found);
    }
    let mut components = path.components();
    let (Some(Component::RootDir), Some(run), Some(user), Some(uid), Some(doc)) = (
        components.next(),
        components.next(),
        components.next(),
        components.next(),
        components.next(),
    ) else {
        return None;
    };
    let is = |c: Component<'_>, name: &str| c.as_os_str() == name;
    let numeric = uid
        .as_os_str()
        .to_str()
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
    if !(is(run, "run") && is(user, "user") && numeric && is(doc, "doc")) {
        return None;
    }
    split_after(&path.components().take(5).collect::<PathBuf>())
}

/// The document id when `path` is under the Documents portal's folder.
fn portal_document(path: &Path, runtime_dir: Option<&Path>) -> Option<String> {
    split_portal(path, runtime_dir).map(|document| document.id)
}

/// The app's [`HostPaths`] (Linux): the Documents portal's `GetHostPaths`
/// over the session bus, then the file's `user.document-portal.host-path`
/// extended attribute where the portal is older (011 research.md §5).
#[cfg(target_os = "linux")]
pub struct PortalHostPaths {
    connection: std::sync::OnceLock<Option<zbus::blocking::Connection>>,
}

#[cfg(target_os = "linux")]
impl PortalHostPaths {
    pub fn new() -> Self {
        Self { connection: std::sync::OnceLock::new() }
    }

    fn ask_portal(&self, document_id: &str) -> Option<PathBuf> {
        use std::os::unix::ffi::OsStrExt;

        let connection = self
            .connection
            .get_or_init(|| {
                zbus::blocking::Connection::session()
                    .map_err(|err| log::warn!("no session bus for the Documents portal: {err}"))
                    .ok()
            })
            .as_ref()?;
        let portal = zbus::blocking::Proxy::new(
            connection,
            "org.freedesktop.portal.Documents",
            "/org/freedesktop/portal/documents",
            "org.freedesktop.portal.Documents",
        )
        .ok()?;
        let paths: HashMap<String, Vec<u8>> =
            portal.call("GetHostPaths", &(vec![document_id.to_owned()],)).ok()?;
        let bytes = paths.get(document_id)?;
        let end = bytes.iter().rposition(|b| *b != 0).map_or(0, |last| last + 1);
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(&bytes[..end]));
        path.is_absolute().then_some(path)
    }
}

#[cfg(target_os = "linux")]
impl Default for PortalHostPaths {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
impl HostPaths for PortalHostPaths {
    fn host_path(&self, portal_path: &Path) -> Option<PathBuf> {
        // The portal's answer and the attribute are both for the document's
        // entry (`doc/<id>/<name>`); the rest below it is joined on.
        ByDocument(|id: &str, entry: &Path| {
            self.ask_portal(id).or_else(|| host_path_attribute(entry))
        })
        .host_path(portal_path)
    }
}

/// The file's `user.document-portal.host-path` extended attribute.
#[cfg(target_os = "linux")]
fn host_path_attribute(portal_path: &Path) -> Option<PathBuf> {
    use std::ffi::{CString, OsStr};
    use std::os::unix::ffi::OsStrExt;

    const NAME: &std::ffi::CStr = c"user.document-portal.host-path";
    let file = CString::new(portal_path.as_os_str().as_bytes()).ok()?;
    // SAFETY: `file` and `NAME` are NUL-terminated and outlive the calls; a
    // null buffer of size 0 only asks for the value's length.
    let size = unsafe { libc::getxattr(file.as_ptr(), NAME.as_ptr(), std::ptr::null_mut(), 0) };
    let mut buffer = vec![0u8; usize::try_from(size).ok().filter(|size| *size > 0)?];
    // SAFETY: as above, and `buffer` is as long as the length passed.
    let read = unsafe {
        libc::getxattr(file.as_ptr(), NAME.as_ptr(), buffer.as_mut_ptr().cast(), buffer.len())
    };
    buffer.truncate(usize::try_from(read).ok()?);
    let end = buffer.iter().rposition(|b| *b != 0).map_or(0, |last| last + 1);
    let path = PathBuf::from(OsStr::from_bytes(&buffer[..end]));
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const HOME: &str = "/home/someone";

    fn home() -> PathBuf {
        PathBuf::from(HOME)
    }

    fn dirs() -> UserDirs {
        UserDirs::parse(
            "# comment\nXDG_DOCUMENTS_DIR=\"$HOME/Dokumente\"\nXDG_DOWNLOAD_DIR=\"${HOME}/Downloads\"\n\
             XDG_DESKTOP_DIR=\"/data/desk top\"\nXDG_PUBLICSHARE_DIR=\"$HOME/Public\"\n",
            &home(),
        )
    }

    fn info(filesystems: &str) -> String {
        format!(
            "[Application]\nname=io.github.exodious.HoploDex\n\n[Context]\nshared=ipc;\nfilesystems={filesystems}\n\n[Session Bus Policy]\nx=y\n"
        )
    }

    fn grants_of(filesystems: &str) -> Vec<Grant> {
        Reach::from_flatpak_info(&info(filesystems), &home(), &dirs()).grants().to_vec()
    }

    fn grant(root: &str, writable: bool) -> Grant {
        Grant { root: PathBuf::from(root), writable }
    }

    // 011 data-model.md "Reach": one test per row of the entry table.

    #[test]
    fn home_and_tilde_grant_the_home_folder_writable() {
        assert_eq!(grants_of("home;"), vec![grant(HOME, true)]);
        assert_eq!(grants_of("~;"), vec![grant(HOME, true)]);
    }

    #[test]
    fn host_grants_the_root_writable() {
        assert_eq!(grants_of("host;"), vec![grant("/", true)]);
    }

    #[test]
    fn host_os_and_host_etc_are_not_counted() {
        assert_eq!(grants_of("host-os;host-etc;host-os:ro"), vec![]);
    }

    #[test]
    fn a_path_under_the_tilde_grants_that_folder_in_home() {
        assert_eq!(grants_of("~/Archive/guns;"), vec![grant("/home/someone/Archive/guns", true)]);
    }

    // Unix only: its paths are POSIX absolute ("/home/..."), which Windows
    // does not count as absolute; a Flatpak runs only on Linux.
    #[cfg(unix)]
    #[test]
    fn user_folders_resolve_through_user_dirs_bare_and_with_a_path() {
        assert_eq!(
            grants_of(
                "xdg-documents;xdg-download:ro;xdg-desktop/sub;xdg-documents/Firearms:create;xdg-public-share"
            ),
            vec![
                grant("/home/someone/Dokumente", true),
                grant("/home/someone/Downloads", false),
                grant("/data/desk top/sub", true),
                grant("/home/someone/Dokumente/Firearms", true),
                grant("/home/someone/Public", true),
            ]
        );
    }

    // Unix only: its paths are POSIX absolute ("/home/..."), which Windows
    // does not count as absolute; a Flatpak runs only on Linux.
    #[cfg(unix)]
    #[test]
    fn a_user_folder_user_dirs_does_not_name_is_not_counted() {
        assert_eq!(
            grants_of("xdg-music;xdg-documents"),
            vec![grant("/home/someone/Dokumente", true)]
        );
        let none = Reach::from_flatpak_info(&info("xdg-documents"), &home(), &UserDirs::default());
        assert_eq!(none.grants(), &[]);
    }

    #[test]
    fn an_absolute_path_grants_that_path() {
        assert_eq!(
            grants_of("/media;/run/media;/mnt/../srv/./x;"),
            vec![grant("/media", true), grant("/run/media", true), grant("/srv/x", true),]
        );
    }

    #[test]
    fn ro_makes_a_root_read_only_and_create_and_rw_make_it_writable() {
        assert_eq!(
            grants_of("home:ro;/media:create;/mnt:rw;/srv:ro"),
            vec![
                grant(HOME, false),
                grant("/media", true),
                grant("/mnt", true),
                grant("/srv", false)
            ]
        );
    }

    #[test]
    fn an_entry_it_does_not_understand_is_not_counted() {
        assert_eq!(grants_of("xdg-run/app;weird;home:bogus;/media;"), vec![grant("/media", true)]);
    }

    // Unix only: its paths are POSIX absolute ("/home/..."), which Windows
    // does not count as absolute; a Flatpak runs only on Linux.
    #[cfg(unix)]
    #[test]
    fn a_removed_entry_takes_away_what_it_would_grant_by_the_longest_match() {
        let reach =
            Reach::from_flatpak_info(&info("home;!~/Private;~/Private/open"), &home(), &dirs());
        assert_eq!(reach.access(Path::new("/home/someone/x")), Access::Writable);
        assert_eq!(reach.access(Path::new("/home/someone/Private")), Access::None);
        assert_eq!(reach.access(Path::new("/home/someone/Private/x")), Access::None);
        // A grant below the removal is closer, so it holds.
        assert_eq!(reach.access(Path::new("/home/someone/Private/open/y")), Access::Writable);
        // An equally close removal wins over a grant, in either order.
        let both = Reach::from_flatpak_info(&info("!/srv;/srv"), &home(), &dirs());
        assert_eq!(both.access(Path::new("/srv/a")), Access::None);
    }

    // 011 research.md §5 and data-model.md "Reach": the file's shape.

    #[test]
    fn grants_come_only_from_the_context_groups_filesystems_key() {
        let text = "[Application]\nfilesystems=/elsewhere\n[Context]\nsockets=x11;\nfilesystems=home;/mnt;\n[Other]\nfilesystems=/nope\n";
        assert_eq!(
            Reach::from_flatpak_info(text, &home(), &dirs()).grants(),
            &[grant(HOME, true), grant("/mnt", true)]
        );
    }

    #[test]
    fn a_file_without_a_context_group_or_filesystems_key_gives_flatpak_with_no_grants() {
        for text in
            ["", "[Application]\nname=x\n", "[Context]\nshared=ipc;\n", "[Context]\nfilesystems=\n"]
        {
            let reach = Reach::from_flatpak_info(text, &home(), &dirs());
            assert!(reach.is_flatpak(), "{text:?}");
            assert!(reach.grants().is_empty(), "{text:?}");
        }
    }

    #[test]
    fn detect_with_no_file_is_unconfined() {
        let dir = TempDir::new().unwrap();
        let reach = Reach::detect(
            &dir.path().join(".flatpak-info"),
            &home(),
            &dirs(),
            Arc::new(NoHostPaths),
        );
        assert!(!reach.is_flatpak());
    }

    #[test]
    fn detect_reads_the_file_and_an_unreadable_one_gives_flatpak_with_no_grants() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join(".flatpak-info");
        std::fs::write(&file, info("home;/mnt")).unwrap();
        let reach = Reach::detect(&file, &home(), &dirs(), Arc::new(NoHostPaths));
        assert!(reach.is_flatpak());
        assert_eq!(reach.grants(), &[grant(HOME, true), grant("/mnt", true)]);

        // A directory where the file should be can't be read as text.
        let unreadable = dir.path().join("dir-info");
        std::fs::create_dir(&unreadable).unwrap();
        let reach = Reach::detect(&unreadable, &home(), &dirs(), Arc::new(NoHostPaths));
        assert!(reach.is_flatpak());
        assert!(reach.grants().is_empty());
    }

    // Unix only: its paths are POSIX absolute ("/home/..."), which Windows
    // does not count as absolute; a Flatpak runs only on Linux.
    #[cfg(unix)]
    #[test]
    fn user_dirs_reads_the_config_homes_file_and_a_missing_one_gives_none() {
        let dir = TempDir::new().unwrap();
        assert!(UserDirs::read(dir.path(), &home()).folder("documents").is_none());
        std::fs::write(
            dir.path().join("user-dirs.dirs"),
            "XDG_DOCUMENTS_DIR=\"$HOME/Docs\"\nXDG_MUSIC_DIR=\"relative\"\n",
        )
        .unwrap();
        let read = UserDirs::read(dir.path(), &home());
        assert_eq!(read.folder("documents"), Some(Path::new("/home/someone/Docs")));
        assert_eq!(read.folder("music"), None);
    }

    // 011 research.md §5: how paths are compared and what `check` decides.

    struct Fixed(HashMap<PathBuf, PathBuf>);

    impl HostPaths for Fixed {
        fn host_path(&self, portal_path: &Path) -> Option<PathBuf> {
            self.0.get(portal_path).cloned()
        }
    }

    struct World {
        _dir: TempDir,
        root: PathBuf,
        reach: Reach,
    }

    /// A Flatpak whose home is `<tmp>/home` (writable), with `<tmp>/ro`
    /// read-only, and `<tmp>/outside` granted nothing.
    fn world(host_map: impl FnOnce(&Path) -> Vec<(&'static str, PathBuf)>) -> World {
        world_with(|root| {
            let map = host_map(root)
                .into_iter()
                .map(|(portal, host)| (PathBuf::from(portal), host))
                .collect();
            Arc::new(Fixed(map))
        })
    }

    /// As [`world`], with any lookup of the Documents portal.
    fn world_with(host: impl FnOnce(&Path) -> Arc<dyn HostPaths>) -> World {
        let dir = TempDir::new().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        for sub in ["home", "ro", "outside"] {
            std::fs::create_dir(root.join(sub)).unwrap();
        }
        std::fs::write(root.join("home/inside.txt"), "x").unwrap();
        std::fs::write(root.join("outside/real.txt"), "x").unwrap();
        let host_paths = host(&root);
        let reach = Reach::flatpak(
            vec![
                Grant { root: root.join("home"), writable: true },
                Grant { root: root.join("ro"), writable: false },
            ],
            Vec::new(),
            root.join("home"),
            host_paths,
        );
        World { _dir: dir, root, reach }
    }

    fn listing(dir: &Path) -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                out.push(path.clone());
                if path.is_dir() {
                    walk(&path, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, &mut out);
        out.sort();
        out
    }

    #[test]
    fn dot_dot_is_normalised_without_following_links_and_the_root_is_within_its_grant() {
        let w = world(|_| vec![]);
        let home = w.root.join("home");
        assert_eq!(w.reach.access(&home), Access::Writable);
        assert_eq!(w.reach.access(&home.join("a/./b")), Access::Writable);
        assert_eq!(w.reach.access(&home.join("a/../../outside")), Access::None);
        assert_eq!(w.reach.access(&home.join("a/../b")), Access::Writable);
        assert_eq!(w.reach.access(&w.root.join("ro/x")), Access::ReadOnly);
        // A sibling that merely starts with the same letters is not within.
        assert_eq!(w.reach.access(&w.root.join("home2/x")), Access::None);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_inside_home_to_outside_is_judged_by_its_own_path() {
        let w = world(|_| vec![]);
        let link = w.root.join("home/link");
        std::os::unix::fs::symlink(w.root.join("outside"), &link).unwrap();
        assert_eq!(w.reach.access(&link.join("real.txt")), Access::Writable);
    }

    #[test]
    fn a_database_needs_its_folder_in_a_writable_grant() {
        let w = world(|_| vec![]);
        let ok = w.root.join("home/a.hoplodex");
        assert_eq!(w.reach.check(&ok, Need::DatabaseFolder).unwrap(), ok);
        for bad in ["ro/a.hoplodex", "outside/a.hoplodex", "home/../outside/a.hoplodex"] {
            let err = w.reach.check(&w.root.join(bad), Need::DatabaseFolder).unwrap_err();
            assert_eq!(err.code, "LOCATION_OUT_OF_REACH", "{bad}");
        }
        // The folder, not the file: the home folder's own file is within.
        assert!(
            w.reach.check(&w.root.join("home/new/deeper/a.hoplodex"), Need::DatabaseFolder).is_ok()
        );
    }

    #[test]
    fn a_write_folder_is_within_a_writable_grant_itself() {
        let w = world(|_| vec![]);
        assert!(w.reach.check(&w.root.join("home"), Need::WriteFolder).is_ok());
        assert!(w.reach.check(&w.root.join("home/backups"), Need::WriteFolder).is_ok());
        for bad in ["ro", "ro/backups", "outside", "outside/backups"] {
            let err = w.reach.check(&w.root.join(bad), Need::WriteFolder).unwrap_err();
            assert_eq!(err.code, "LOCATION_OUT_OF_REACH", "{bad}");
        }
    }

    /// 011 T096, 011 FR-007: a file is within reach only where a grant covers
    /// it, whether or not it exists in the sandbox. A drop from outside reach
    /// is always refused, even where the same path shows up in the sandbox.
    #[test]
    fn a_file_to_read_is_accepted_only_inside_a_grant_whether_or_not_it_exists() {
        let w = world(|_| vec![]);
        for ok in ["home/inside.txt", "home/missing.txt", "ro/missing.txt"] {
            let path = w.root.join(ok);
            assert_eq!(w.reach.check(&path, Need::ReadFile).unwrap(), path, "{ok}");
        }
        for (refused, folder) in
            [("outside/real.txt", "outside"), ("outside/missing.txt", "outside")]
        {
            let err = w.reach.check(&w.root.join(refused), Need::ReadFile).unwrap_err();
            assert_eq!(err.code, "LOCATION_OUT_OF_REACH", "{refused}");
            assert_eq!(
                err.details.as_ref().unwrap()["folder"],
                w.root.join(folder).to_string_lossy().as_ref(),
                "{refused}"
            );
        }
    }

    #[test]
    fn unconfined_returns_every_path_unchanged_and_touches_nothing() {
        let dir = TempDir::new().unwrap();
        let before = listing(dir.path());
        for need in [Need::DatabaseFolder, Need::WriteFolder, Need::ReadFile] {
            for path in
                [dir.path().join("a/b"), PathBuf::from("/nowhere/x"), PathBuf::from("relative/x")]
            {
                assert_eq!(Reach::Unconfined.check(&path, need).unwrap(), path);
            }
        }
        assert_eq!(listing(dir.path()), before);
        assert!(!Reach::Unconfined.is_flatpak());
    }

    const DOC: &str = "/run/user/1000/doc/a1b2c3/pick.hoplodex";

    // Unix only: its paths are POSIX absolute ("/home/..."), which Windows
    // does not count as absolute; a Flatpak runs only on Linux.
    #[cfg(unix)]
    #[test]
    fn portal_paths_are_recognised_under_run_user_and_the_runtime_dir() {
        let doc =
            |p: &str, runtime: Option<&str>| portal_document(Path::new(p), runtime.map(Path::new));
        assert_eq!(doc(DOC, None).as_deref(), Some("a1b2c3"));
        assert_eq!(doc("/run/user/1000/doc/a1b2c3", None).as_deref(), Some("a1b2c3"));
        assert_eq!(doc("/xdg/rt/doc/zz/f", Some("/xdg/rt")).as_deref(), Some("zz"));
        assert_eq!(doc("/run/user/1000/doc", None), None);
        assert_eq!(doc("/run/user/abc/doc/x/f", None), None);
        assert_eq!(doc("/run/user/1000/gvfs/x/f", None), None);
        assert_eq!(doc("/home/x/doc/y/f", None), None);
    }

    /// The cells of 011 data-model.md's resolution table.
    // Unix only: a portal path ("/run/user/<uid>/doc/...") is made absolute
    // with a drive letter on Windows, where there is no Documents portal.
    #[cfg(unix)]
    #[test]
    fn a_portal_path_resolved_into_a_writable_grant_gives_its_host_path_for_every_need() {
        let w = world(|root| vec![(DOC, root.join("home/pick.hoplodex"))]);
        let expected = w.root.join("home/pick.hoplodex");
        for need in [Need::DatabaseFolder, Need::WriteFolder, Need::ReadFile] {
            assert_eq!(w.reach.check(Path::new(DOC), need).unwrap(), expected, "{need:?}");
        }
    }

    // Unix only: a portal path ("/run/user/<uid>/doc/...") is made absolute
    // with a drive letter on Windows, where there is no Documents portal.
    #[cfg(unix)]
    #[test]
    fn a_portal_path_resolved_into_a_read_only_grant_is_read_but_not_written() {
        let w = world(|root| vec![(DOC, root.join("ro/pick.hoplodex"))]);
        let read = w.reach.check(Path::new(DOC), Need::ReadFile).unwrap();
        assert_eq!(read, w.root.join("ro/pick.hoplodex"));
        for need in [Need::DatabaseFolder, Need::WriteFolder] {
            let err = w.reach.check(Path::new(DOC), need).unwrap_err();
            assert_eq!(err.code, "LOCATION_OUT_OF_REACH", "{need:?}");
        }
    }

    // Unix only: a portal path ("/run/user/<uid>/doc/...") is made absolute
    // with a drive letter on Windows, where there is no Documents portal.
    #[cfg(unix)]
    #[test]
    fn a_portal_path_outside_every_grant_or_unresolved_is_kept_except_for_a_database() {
        for resolved in [true, false] {
            let w = world(|_| {
                if resolved {
                    vec![(DOC, PathBuf::from("/media/elsewhere/pick.hoplodex"))]
                } else {
                    vec![]
                }
            });
            let err = w.reach.check(Path::new(DOC), Need::DatabaseFolder).unwrap_err();
            assert_eq!(err.code, "LOCATION_OUT_OF_REACH");
            for need in [Need::WriteFolder, Need::ReadFile] {
                assert_eq!(
                    w.reach.check(Path::new(DOC), need).unwrap(),
                    Path::new(DOC),
                    "{need:?}"
                );
            }
        }
    }

    // 011 T094: a lookup that answers by document id, as the Documents portal
    // does. The test doubles above map whole paths, which hid a path below the
    // document being dropped.

    /// A lookup by document id: `id` is the document's host path, which stands
    /// for `doc/<id>/<name>`.
    fn by_document(docs: Vec<(&'static str, PathBuf)>) -> Arc<dyn HostPaths> {
        let docs: HashMap<String, PathBuf> =
            docs.into_iter().map(|(id, host)| (id.to_owned(), host)).collect();
        Arc::new(ByDocument(move |id: &str, _entry: &Path| docs.get(id).cloned()))
    }

    const DOCS: &str = "/run/user/1000/doc";

    // Unix only: portal paths are POSIX absolute ("/run/user/..."), which Windows
    // does not count as absolute; a Flatpak runs only on Linux.
    #[cfg(unix)]
    #[test]
    fn a_portal_path_splits_into_the_document_its_entry_and_what_lies_below() {
        let split = |p: &str, runtime: Option<&str>| {
            split_portal(Path::new(p), runtime.map(Path::new))
                .map(|d| (d.id, d.entry, d.below))
                .map(|(id, entry, below)| {
                    (id, entry.display().to_string(), below.display().to_string())
                })
        };
        let triple = |id: &str, entry: &str, below: &str| {
            Some((id.to_owned(), entry.to_owned(), below.to_owned()))
        };
        assert_eq!(split(DOC, None), triple("a1b2c3", &format!("{DOCS}/a1b2c3/pick.hoplodex"), ""));
        assert_eq!(
            split("/run/user/1000/doc/f1/Guns/sub/a.hoplodex", None),
            triple("f1", &format!("{DOCS}/f1/Guns"), "sub/a.hoplodex")
        );
        assert_eq!(
            split("/xdg/rt/doc/f1/Guns/a.hoplodex", Some("/xdg/rt")),
            triple("f1", "/xdg/rt/doc/f1/Guns", "a.hoplodex")
        );
        assert_eq!(split("/run/user/1000/doc/f1", None), triple("f1", &format!("{DOCS}/f1"), ""));
        assert_eq!(split("/run/user/1000/doc", None), None);
        assert_eq!(split("/home/x/doc/y/f", None), None);
    }

    // Unix only: see above.
    #[cfg(unix)]
    #[test]
    fn a_folder_documents_host_path_gets_what_lies_below_it_joined_on() {
        let host = by_document(vec![
            ("f1", PathBuf::from("/h/Guns")),
            ("f2", PathBuf::from("/h/Mine.hoplodex")),
        ]);
        let resolve = |p: &str| host.host_path(Path::new(p));
        // A picked folder: the database in it, in a folder below it, the folder.
        assert_eq!(
            resolve("/run/user/1000/doc/f1/Guns/a.hoplodex"),
            Some("/h/Guns/a.hoplodex".into())
        );
        assert_eq!(
            resolve("/run/user/1000/doc/f1/Guns/sub/deeper/a.hoplodex"),
            Some("/h/Guns/sub/deeper/a.hoplodex".into())
        );
        assert_eq!(resolve("/run/user/1000/doc/f1/Guns"), Some("/h/Guns".into()));
        // A picked file stands for itself.
        assert_eq!(resolve("/run/user/1000/doc/f2/Mine.hoplodex"), Some("/h/Mine.hoplodex".into()));
        // Another document, and a `..` that climbs out of the portal's folder.
        assert_eq!(resolve("/run/user/1000/doc/nope/x"), None);
        assert_eq!(resolve("/run/user/1000/doc/f1/Guns/../../../x"), None);
        // A `..` inside the document stays inside it.
        assert_eq!(resolve("/run/user/1000/doc/f1/Guns/a/../b"), Some("/h/Guns/b".into()));
    }

    /// 011 T094, 011 FR-003, 011 US1-3: a database created in a folder the
    /// chooser picked opens, and is made, at the host path below the picked
    /// folder, not at the picked folder itself.
    // Unix only: see above.
    #[cfg(unix)]
    #[test]
    fn a_path_below_a_picked_folder_resolves_below_its_host_folder_for_every_need() {
        let w = world_with(|root| {
            by_document(vec![
                ("f1", root.join("home/Guns")),
                ("f4", root.join("ro/Docs")),
                ("f5", root.join("home/Mine.hoplodex")),
            ])
        });
        let home = w.root.join("home");
        let picked = |tail: &str| PathBuf::from(format!("{DOCS}/{tail}"));

        for (portal, need, expected) in [
            ("f1/Guns/Mine.hoplodex", Need::DatabaseFolder, home.join("Guns/Mine.hoplodex")),
            (
                "f1/Guns/a/b/Mine.hoplodex",
                Need::DatabaseFolder,
                home.join("Guns/a/b/Mine.hoplodex"),
            ),
            ("f1/Guns/Exports", Need::WriteFolder, home.join("Guns/Exports")),
            ("f1/Guns/Backups", Need::BackupFolder, home.join("Guns/Backups")),
            ("f1/Guns/sheet.csv", Need::ReadFile, home.join("Guns/sheet.csv")),
            ("f5/Mine.hoplodex", Need::DatabaseFolder, home.join("Mine.hoplodex")),
            ("f4/Docs/receipt.pdf", Need::ReadFile, w.root.join("ro/Docs/receipt.pdf")),
        ] {
            assert_eq!(
                w.reach.check(&picked(portal), need).unwrap(),
                expected,
                "{portal} {need:?}"
            );
        }
    }

    /// 011 T094, 011 T095: below a picked folder outside every grant, a database
    /// is refused naming the host path and the host folder (so the override
    /// command names a folder that helps), and a read-only grant refuses
    /// writing the same way.
    // Unix only: see above.
    #[cfg(unix)]
    #[test]
    fn a_refused_portal_path_names_the_host_path_and_the_host_folder() {
        let w = world_with(|root| {
            by_document(vec![
                ("f2", root.join("outside/Guns")),
                ("f3", root.join("outside/Theirs.hoplodex")),
                ("f4", root.join("ro/Docs")),
            ])
        });
        let picked = |tail: &str| PathBuf::from(format!("{DOCS}/{tail}"));
        let refusal = |tail: &str, need: Need| {
            let err = w.reach.check(&picked(tail), need).unwrap_err();
            assert_eq!(err.code, "LOCATION_OUT_OF_REACH", "{tail} {need:?}");
            let details = err.details.as_ref().unwrap();
            (details["path"].clone(), details["folder"].clone(), err.message.clone())
        };
        let text = |p: PathBuf| serde_json::Value::from(p.to_string_lossy().into_owned());

        let (path, folder, message) = refusal("f2/Guns/sub/Mine.hoplodex", Need::DatabaseFolder);
        assert_eq!(path, text(w.root.join("outside/Guns/sub/Mine.hoplodex")));
        assert_eq!(folder, text(w.root.join("outside/Guns/sub")));
        assert!(
            message
                .contains(&format!("--filesystem={}", w.root.join("outside/Guns/sub").display()))
        );

        // A picked file: its host folder.
        let (path, folder, _) = refusal("f3/Theirs.hoplodex", Need::DatabaseFolder);
        assert_eq!(path, text(w.root.join("outside/Theirs.hoplodex")));
        assert_eq!(folder, text(w.root.join("outside")));

        // Read-only: refused for writing, the host folder named.
        let (path, folder, _) = refusal("f4/Docs/Backups", Need::BackupFolder);
        assert_eq!(path, text(w.root.join("ro/Docs/Backups")));
        assert_eq!(folder, text(w.root.join("ro/Docs/Backups")));
        let (_, folder, _) = refusal("f4/Docs/Mine.hoplodex", Need::DatabaseFolder);
        assert_eq!(folder, text(w.root.join("ro/Docs")));

        // The portal's own folder is named only when nothing resolves it.
        let (path, folder, _) = refusal("f9/Mine.hoplodex", Need::DatabaseFolder);
        assert_eq!(path, text(picked("f9/Mine.hoplodex")));
        assert_eq!(folder, text(picked("f9")));
    }

    /// 011 T097: a Documents-portal path outside reach is no stored backup
    /// location (the host folder is named, or the portal's when nothing
    /// resolves), while a one-off write (an export) and a read keep it.
    // Unix only: see above.
    #[cfg(unix)]
    #[test]
    fn a_portal_path_outside_reach_is_refused_as_a_backup_folder_but_kept_for_a_one_off_write() {
        let w = world_with(|root| {
            by_document(vec![("f1", root.join("home/Guns")), ("f2", root.join("outside/Guns"))])
        });
        let picked = |tail: &str| PathBuf::from(format!("{DOCS}/{tail}"));

        // Within reach it is the host path, as for a one-off write.
        assert_eq!(
            w.reach.check(&picked("f1/Guns"), Need::BackupFolder).unwrap(),
            w.root.join("home/Guns")
        );

        // Outside every grant, or unresolved: refused with the folder the
        // override would need.
        let err = w.reach.check(&picked("f2/Guns"), Need::BackupFolder).unwrap_err();
        assert_eq!(err.code, "LOCATION_OUT_OF_REACH");
        assert_eq!(
            err.details.as_ref().unwrap()["folder"],
            w.root.join("outside/Guns").to_string_lossy().as_ref()
        );
        let err = w.reach.check(&picked("f9/Guns"), Need::BackupFolder).unwrap_err();
        assert_eq!(err.code, "LOCATION_OUT_OF_REACH");

        // The one-off write and the read keep the portal path.
        for tail in ["f2/Guns", "f9/Guns"] {
            assert_eq!(w.reach.check(&picked(tail), Need::WriteFolder).unwrap(), picked(tail));
            assert_eq!(w.reach.check(&picked(tail), Need::ReadFile).unwrap(), picked(tail));
        }

        // A plain path is judged as a write folder is.
        assert!(w.reach.check(&w.root.join("home/Backups"), Need::BackupFolder).is_ok());
        for bad in ["ro/Backups", "outside/Backups"] {
            let err = w.reach.check(&w.root.join(bad), Need::BackupFolder).unwrap_err();
            assert_eq!(err.code, "LOCATION_OUT_OF_REACH", "{bad}");
        }
    }

    // 011 T112: where /home links to /var/home (Fedora Silverblue) the grant
    // names one and the user types the other.

    /// `<tmp>/var/home/someone` (with `Archive` and `Private`), `<tmp>/home`
    /// linking to `var/home`, a real `<tmp>/outside`, and `<tmp>/elsewhere/lnk`
    /// linking into the home folder.
    #[cfg(unix)]
    struct Linked {
        _dir: TempDir,
        root: PathBuf,
    }

    #[cfg(unix)]
    impl Linked {
        fn new() -> Self {
            let dir = TempDir::new().unwrap();
            let root = std::fs::canonicalize(dir.path()).unwrap();
            std::fs::create_dir_all(root.join("var/home/someone/Archive")).unwrap();
            std::fs::create_dir_all(root.join("var/home/someone/Private")).unwrap();
            std::fs::create_dir_all(root.join("outside")).unwrap();
            std::fs::create_dir_all(root.join("elsewhere")).unwrap();
            std::os::unix::fs::symlink(root.join("var/home"), root.join("home")).unwrap();
            std::os::unix::fs::symlink(
                root.join("var/home/someone/Archive"),
                root.join("elsewhere/lnk"),
            )
            .unwrap();
            std::os::unix::fs::symlink(root.join("outside"), root.join("var/home/someone/out"))
                .unwrap();
            Self { _dir: dir, root }
        }

        fn reach(&self, grant: &str, removed: &[&str]) -> Reach {
            Reach::flatpak(
                vec![Grant { root: self.root.join(grant), writable: true }],
                removed.iter().map(|r| self.root.join(r)).collect(),
                self.root.join(grant),
                Arc::new(NoHostPaths),
            )
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_path_through_a_link_above_the_grant_is_judged_against_the_grants_real_path() {
        let l = Linked::new();
        // The grant is where the sandbox sees the home folder; the user types
        // the other spelling.
        let reach = l.reach("var/home/someone", &[]);
        assert!(reach.check(&l.root.join("home/someone"), Need::WriteFolder).is_ok());
        for ok in [
            "home/someone/a.hoplodex",
            "home/someone/Archive/a.hoplodex",
            "home/someone/new/deeper/a",
        ] {
            assert_eq!(
                reach.check(&l.root.join(ok), Need::DatabaseFolder).unwrap(),
                l.root.join(ok),
                "{ok}"
            );
        }
        assert!(reach.check(&l.root.join("home/someone/Archive"), Need::WriteFolder).is_ok());
        assert!(reach.check(&l.root.join("home/someone/x.csv"), Need::ReadFile).is_ok());
        // The other way round, and a grant that does not exist yet.
        let by_link = l.reach("home/someone", &[]);
        assert!(by_link.check(&l.root.join("var/home/someone/a/b"), Need::WriteFolder).is_ok());
        let ghost = l.reach("var/home/ghost", &[]);
        assert!(ghost.check(&l.root.join("home/ghost/x/a"), Need::DatabaseFolder).is_ok());

        // Neighbours are still outside.
        for bad in ["home/other/a", "home/someone2/a", "home/a", "outside/a", "home/../outside/a"] {
            let err = reach.check(&l.root.join(bad), Need::DatabaseFolder).unwrap_err();
            assert_eq!(err.code, "LOCATION_OUT_OF_REACH", "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_inside_a_grant_is_still_judged_by_its_own_path_and_one_outside_is_not_followed() {
        let l = Linked::new();
        let reach = l.reach("var/home/someone", &[]);
        // `out` links to `<tmp>/outside` from inside the grant: judged by its
        // own path, as bwrap binds it (and as the unlinked case is).
        assert_eq!(reach.access(&l.root.join("home/someone/out/real.txt")), Access::Writable);
        // A link outside the grant that leads into it is not followed.
        assert_eq!(reach.access(&l.root.join("elsewhere/lnk/a")), Access::None);
        // Nor is the real path outside it reached through a link inside.
        assert_eq!(reach.access(&l.root.join("outside/real.txt")), Access::None);
    }

    #[cfg(unix)]
    #[test]
    fn a_removed_folder_is_removed_whichever_way_it_is_spelled() {
        let l = Linked::new();
        let reach = l.reach("var/home/someone", &["var/home/someone/Private"]);
        assert_eq!(reach.access(&l.root.join("home/someone/Archive/a")), Access::Writable);
        assert_eq!(reach.access(&l.root.join("home/someone/Private/a")), Access::None);
        assert_eq!(reach.access(&l.root.join("var/home/someone/Private/a")), Access::None);
        let by_link = l.reach("home/someone", &["home/someone/Private"]);
        assert_eq!(by_link.access(&l.root.join("var/home/someone/Private/a")), Access::None);
        assert_eq!(by_link.access(&l.root.join("var/home/someone/Archive/a")), Access::Writable);
    }

    #[test]
    fn check_writes_nothing() {
        let w = world(|_| vec![(DOC, PathBuf::from("/media/elsewhere/pick.hoplodex"))]);
        let before = listing(&w.root);
        for need in [Need::DatabaseFolder, Need::WriteFolder, Need::ReadFile] {
            for path in [
                w.root.join("home/new/a.hoplodex"),
                w.root.join("ro/new/a.hoplodex"),
                w.root.join("outside/new/a.hoplodex"),
                PathBuf::from(DOC),
            ] {
                let _ = w.reach.check(&path, need);
            }
        }
        assert_eq!(listing(&w.root), before);
    }

    #[test]
    fn the_refusal_names_the_path_the_folder_and_the_grants() {
        let w = world(|_| vec![]);
        let path = w.root.join("outside/new/a.hoplodex");
        let err = w.reach.check(&path, Need::DatabaseFolder).unwrap_err();
        let details = err.details.as_ref().unwrap();
        assert_eq!(details["path"], path.to_string_lossy().as_ref());
        assert_eq!(details["folder"], w.root.join("outside/new").to_string_lossy().as_ref());
        assert_eq!(details["grants"].as_array().unwrap().len(), 2);
    }
}
