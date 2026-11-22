//! What the user handed the app outside the page: the files dropped onto the
//! main window, and the files and folder picked in a chooser Rust opened
//! (code scanning alerts `paths/cwe-73` on `add_document_from_path` and
//! `export_collection`, amended 2026-11-19 to 001 FR-010, FR-018 and FR-019).
//!
//! A command that reads or writes a file the user chose takes it from here,
//! never from a path the page names: a compromised page can then reach only
//! what the user dropped or picked, not any file it can spell. Everything is
//! bound to the [`SessionId`] it arrived in and used once. A drop is
//! short-lived as well, since the page attaches it as it arrives. A lock,
//! close or switch forgets all of it (`Session::forget_open`).

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, TimeDelta};

use super::SessionId;
use crate::commands::CommandError;

/// How long a dropped file stays attachable. The page attaches each drop as
/// it arrives, one file after another, so this only has to outlast a drop of
/// many large files.
pub const DROP_LIFETIME: TimeDelta = TimeDelta::minutes(2);

/// The most dropped files held at once; the oldest go first. A drop of more
/// is far beyond any record's limits (`MAX_PHOTOS_PER_RECORD`,
/// `MAX_DOCUMENTS_PER_RECORD`).
pub const MAX_DROPS: usize = 256;

/// The most chooser answers held at once; the oldest go first.
pub const MAX_PICKS: usize = 16;

/// What a chooser answer may be used for, so an answer to one question can't
/// answer another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    /// A spreadsheet to import (`choose_import_files`).
    ImportFile,
    /// The folder to export to (`choose_export_folder`).
    ExportFolder,
    /// The folder a new database goes in (`choose_database_folder`).
    DatabaseFolder,
    /// The open database's new backup location (`choose_backup_folder`).
    BackupFolder,
    /// The folder for the one backup before an upgrade
    /// (`choose_upgrade_backup_folder`), held under the waiting database's id.
    UpgradeBackupFolder,
    /// A database file to open (`choose_database_file`), taken by its path,
    /// since the chooser's list shows and opens it by path.
    DatabaseFile,
}

struct Dropped {
    session: SessionId,
    path: PathBuf,
    at: DateTime<FixedOffset>,
}

struct Picked {
    session: SessionId,
    kind: PickKind,
    token: String,
    path: PathBuf,
}

/// The drops and chooser answers not yet used, behind a mutex of their own,
/// so a drop is recorded on the main thread without waiting for the open
/// database's.
#[derive(Default)]
pub struct Grants {
    drops: Vec<Dropped>,
    picks: Vec<Picked>,
}

/// The fixed refusal of a file that didn't arrive as a drop: the same for a
/// file that exists and one that doesn't, so it tells the page nothing.
pub fn not_dropped() -> CommandError {
    CommandError::new(
        "VALIDATION_ERROR",
        "That file wasn't dropped onto HoploDex. Drop it again, or choose it with the button.",
    )
}

/// The fixed refusal of a token that isn't a held chooser answer.
pub fn not_picked() -> CommandError {
    CommandError::new("VALIDATION_ERROR", "Choose it again: that choice is no longer available.")
}

/// Whether `path` names a server or a device rather than a place on this
/// computer's own drives: `\\server\share`, `\\?\`, `\\.\`, the NT
/// namespace's `\??\` and their forward-slash spellings. Only a location Rust
/// holds (a chooser answer, the recent list, a folder it derived) may take
/// that form; one the page names is refused before anything looks at it,
/// since even a look reaches the server. Checked by spelling, on every OS, so it is the same everywhere.
pub fn is_server_or_device(path: &Path) -> bool {
    let text = path.as_os_str().to_string_lossy();
    let mut start = text.chars();
    matches!(
        (start.next(), start.next(), start.next(), start.next()),
        (Some('\\' | '/'), Some('\\' | '/'), ..)
            | (Some('\\' | '/'), Some('?'), Some('?'), Some('\\' | '/'))
    )
}

/// The fixed refusal of a server or device path the page named.
pub fn not_a_local_path() -> CommandError {
    CommandError::new(
        "VALIDATION_ERROR",
        "HoploDex takes a network or device location only when you choose it. Choose it again.",
    )
}

impl Grants {
    /// Records the files of one drop for `session`, which is the open one.
    pub fn record_drop(
        &mut self,
        session: SessionId,
        paths: Vec<PathBuf>,
        now: DateTime<FixedOffset>,
    ) {
        self.forget_stale(now);
        self.drops.extend(paths.into_iter().map(|path| Dropped { session, path, at: now }));
        let over = self.drops.len().saturating_sub(MAX_DROPS);
        self.drops.drain(..over);
    }

    /// Takes one drop of `path` in `session`, if one is held and still fresh.
    pub fn take_drop(
        &mut self,
        session: SessionId,
        path: &Path,
        now: DateTime<FixedOffset>,
    ) -> Result<PathBuf, CommandError> {
        self.forget_stale(now);
        let found = self.drops.iter().position(|d| d.session == session && d.path == path);
        found.map(|at| self.drops.remove(at).path).ok_or_else(not_dropped)
    }

    /// Holds a chooser answer for `session` under a new token.
    pub fn hold_pick(
        &mut self,
        session: SessionId,
        kind: PickKind,
        path: PathBuf,
        token: String,
    ) -> String {
        self.picks.push(Picked { session, kind, token: token.clone(), path });
        let over = self.picks.len().saturating_sub(MAX_PICKS);
        self.picks.drain(..over);
        token
    }

    /// Takes the answer held under `token` for `session` and `kind`.
    pub fn take_pick(
        &mut self,
        session: SessionId,
        kind: PickKind,
        token: &str,
    ) -> Result<PathBuf, CommandError> {
        let found = self
            .picks
            .iter()
            .position(|p| p.session == session && p.kind == kind && p.token == token);
        found.map(|at| self.picks.remove(at).path).ok_or_else(not_picked)
    }

    /// Takes the answer for `session` and `kind` whose path is `path`, and
    /// returns its token.
    pub fn take_pick_at(
        &mut self,
        session: SessionId,
        kind: PickKind,
        path: &Path,
    ) -> Result<String, CommandError> {
        let found = self
            .picks
            .iter()
            .position(|p| p.session == session && p.kind == kind && p.path == path);
        found.map(|at| self.picks.remove(at).token).ok_or_else(not_picked)
    }

    /// Forgets everything: the session ended.
    pub fn clear(&mut self) {
        self.drops.clear();
        self.picks.clear();
    }

    /// Forgets the drops past their lifetime. Another session's are left to
    /// [`clear`](Self::clear), so a late call from an ended session can't
    /// forget the open one's.
    fn forget_stale(&mut self, now: DateTime<FixedOffset>) {
        // A clock set back counts as stale too.
        self.drops.retain(|d| d.at <= now && now - d.at <= DROP_LIFETIME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> SessionId {
        SessionId::from_request(n).unwrap()
    }

    fn at(seconds: i64) -> DateTime<FixedOffset> {
        DateTime::from_timestamp(1_800_000_000 + seconds, 0).unwrap().fixed_offset()
    }

    #[test]
    fn a_drop_is_taken_once_in_its_own_session() {
        let mut grants = Grants::default();
        grants.record_drop(id(1), vec!["/a/x.pdf".into(), "/a/y.png".into()], at(0));
        assert!(grants.take_drop(id(2), Path::new("/a/x.pdf"), at(1)).is_err());
        assert_eq!(
            grants.take_drop(id(1), Path::new("/a/x.pdf"), at(1)).unwrap(),
            Path::new("/a/x.pdf")
        );
        assert!(grants.take_drop(id(1), Path::new("/a/x.pdf"), at(1)).is_err());
        assert!(grants.take_drop(id(1), Path::new("/a/other.pdf"), at(1)).is_err());
    }

    #[test]
    fn a_drop_goes_stale() {
        let mut grants = Grants::default();
        grants.record_drop(id(1), vec!["/a/x.pdf".into(), "/a/y.pdf".into()], at(0));
        let limit = DROP_LIFETIME.num_seconds();
        assert!(grants.take_drop(id(1), Path::new("/a/x.pdf"), at(limit)).is_ok());
        assert!(grants.take_drop(id(1), Path::new("/a/y.pdf"), at(limit + 1)).is_err());
        // A clock set back is no way to keep one.
        grants.record_drop(id(1), vec!["/a/z.pdf".into()], at(100));
        assert!(grants.take_drop(id(1), Path::new("/a/z.pdf"), at(99)).is_err());
    }

    #[test]
    fn the_same_file_dropped_twice_is_taken_twice() {
        let mut grants = Grants::default();
        grants.record_drop(id(1), vec!["/a/x.pdf".into()], at(0));
        grants.record_drop(id(1), vec!["/a/x.pdf".into()], at(1));
        assert!(grants.take_drop(id(1), Path::new("/a/x.pdf"), at(2)).is_ok());
        assert!(grants.take_drop(id(1), Path::new("/a/x.pdf"), at(2)).is_ok());
        assert!(grants.take_drop(id(1), Path::new("/a/x.pdf"), at(2)).is_err());
    }

    #[test]
    fn drops_are_bounded_and_the_oldest_go() {
        let mut grants = Grants::default();
        let paths = (0..=MAX_DROPS).map(|n| PathBuf::from(format!("/a/{n}.pdf"))).collect();
        grants.record_drop(id(1), paths, at(0));
        assert!(grants.take_drop(id(1), Path::new("/a/0.pdf"), at(0)).is_err());
        assert!(
            grants.take_drop(id(1), &PathBuf::from(format!("/a/{MAX_DROPS}.pdf")), at(0)).is_ok()
        );
    }

    #[test]
    fn a_pick_answers_only_its_own_question_once() {
        let mut grants = Grants::default();
        let token = grants.hold_pick(id(1), PickKind::ExportFolder, "/out".into(), "t1".into());
        assert!(grants.take_pick(id(1), PickKind::ImportFile, &token).is_err());
        assert!(grants.take_pick(id(2), PickKind::ExportFolder, &token).is_err());
        assert!(grants.take_pick(id(1), PickKind::ExportFolder, "t2").is_err());
        assert_eq!(
            grants.take_pick(id(1), PickKind::ExportFolder, &token).unwrap(),
            Path::new("/out")
        );
        assert!(grants.take_pick(id(1), PickKind::ExportFolder, &token).is_err());
    }

    #[test]
    fn a_pick_is_taken_by_its_path_once() {
        let mut grants = Grants::default();
        grants.hold_pick(id(1), PickKind::DatabaseFile, "/d/a.hoplodex".into(), "t1".into());
        assert!(
            grants.take_pick_at(id(1), PickKind::DatabaseFile, Path::new("/d/b.hoplodex")).is_err()
        );
        assert!(
            grants.take_pick_at(id(2), PickKind::DatabaseFile, Path::new("/d/a.hoplodex")).is_err()
        );
        assert!(
            grants.take_pick_at(id(1), PickKind::ExportFolder, Path::new("/d/a.hoplodex")).is_err()
        );
        assert!(
            grants.take_pick_at(id(1), PickKind::DatabaseFile, Path::new("/d/a.hoplodex")).is_ok()
        );
        assert!(
            grants.take_pick_at(id(1), PickKind::DatabaseFile, Path::new("/d/a.hoplodex")).is_err()
        );
    }

    #[test]
    fn server_and_device_paths_are_told_by_their_spelling() {
        for path in [
            r"\\server\share\a.hoplodex",
            r"\\?\C:\a.hoplodex",
            r"\\?\UNC\server\share",
            r"\\.\PhysicalDrive0",
            "//server/share/a.hoplodex",
            r"/\server\share",
            r"\??\UNC\server\share\a.hoplodex",
            r"\??\PhysicalDrive0",
            "/??/UNC/server/share/a.hoplodex",
        ] {
            assert!(is_server_or_device(Path::new(path)), "{path}");
        }
        for path in
            [r"C:\Users\a.hoplodex", "/home/a/a.hoplodex", r"\Users\a", "/??", "/?/a", "a", ""]
        {
            assert!(!is_server_or_device(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn picks_are_bounded_and_go_when_the_session_ends() {
        let mut grants = Grants::default();
        for n in 0..=MAX_PICKS {
            grants.hold_pick(id(1), PickKind::ImportFile, "/f.csv".into(), format!("t{n}"));
        }
        assert!(grants.take_pick(id(1), PickKind::ImportFile, "t0").is_err());
        assert!(grants.take_pick(id(1), PickKind::ImportFile, "t1").is_ok());
        assert!(grants.take_pick(id(2), PickKind::ImportFile, "t2").is_err());
        grants.clear();
        assert!(grants.take_pick(id(1), PickKind::ImportFile, "t2").is_err());
    }
}
