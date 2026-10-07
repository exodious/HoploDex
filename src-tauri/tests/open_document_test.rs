//! Opening a document in another app (feature 007, T083; FR-008 to FR-012,
//! FR-017, SC-004; research.md §16-§18; contracts/tauri-commands.md
//! "open_document"; ui contract §4). Real SQLCipher databases and real files
//! in temp directories. The native dialog is a recording `Consent` that
//! answers as told, and the OS's opener is a recording `Opener` that can fail.
//!
//! Written ahead of T087-T089; it fails to compile until they exist. Assumed
//! API:
//! - `services::consent::{Consent, ConsentRequest, ConsentAnswer,
//!   needs_consent}`:
//!   - `trait Consent: Send + Sync { fn ask(&self, request: ConsentRequest)
//!     -> ConsentAnswer; }`;
//!   - `enum ConsentAnswer { Open, Cancel }`;
//!   - `#[derive(Clone, Debug)] enum ConsentRequest { Document { name: String,
//!     kind: String, first_of_session_with_external: bool }, Setting }`, with
//!     `fn title(&self) -> String` and `fn body(&self) -> String` (ui
//!     contract §4); `kind` is the document type's label ("PDF", "Word");
//!   - `fn needs_consent(setting: DocumentOpening, confirmed_this_session:
//!     bool) -> bool`;
//! - `commands::documents::{Opener, OpenFailure}`: `trait Opener: Send + Sync
//!   { fn open(&self, path: &Path) -> Result<(), OpenFailure>; }` and
//!   `enum OpenFailure { NoApp, Other(String) }`;
//! - `commands::documents::ops::open_document(session: &Session, machine:
//!   &MachineSettings, consent: &dyn Consent, opener: &dyn Opener,
//!   opened_documents_dir: &Path, id: i64) -> Result<OpenedDocument,
//!   CommandError>`, `OpenedDocument` serializing as `{ "opened": bool }`,
//!   `opened_documents_dir` being the root that holds `<id>/<name>` (the one
//!   the session clears at a close);
//! - `session::OpenDatabase { generation: u64, external_open_confirmed: bool }`;
//! - the codes `NO_APP_FOR_DOCUMENT`, `DOCUMENT_TYPE_NOT_ALLOWED`,
//!   `DOCUMENT_CONTENT_MISMATCH`, `DATABASE_CLOSED`, `INTERNAL_ERROR`,
//!   `NOT_FOUND`.
//!
//! The close-out tasks cite `issue_65_*` (the copy is written under the
//! session lock and only into the session that asked) and `issue_71_*` (Unix
//! modes from creation, the symlink and owner check).
//!
//! Room for T090 (macOS: the `com.apple.quarantine` attribute) and T091
//! (Windows: the folder's DACL and the `:Zone.Identifier` stream), which the
//! macOS and Windows sessions add as `#[cfg(...)]` cases at the end of this
//! file, and for T095 (the setting's own request).

#[path = "support/preview_support.rs"]
mod preview_support;
mod support;

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::documents::{OpenFailure, Opener};
use hoplodex_lib::commands::preview::ops as preview;
use hoplodex_lib::models::database::{CloseReason, LockSettingsInput};
use hoplodex_lib::models::document_opening::DocumentOpening;
use hoplodex_lib::services::consent::{Consent, ConsentAnswer, ConsentRequest, needs_consent};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::preview::SurfaceBounds;
use hoplodex_lib::services::preview::protocol_handler;
use hoplodex_lib::session::{Session, lifecycle};
use preview_support::{TestPreview, add_document, insert_raw_document, new_firearm};
use serde_json::Value;
use support::hostile_documents as hostile;
use support::{ManualClock, TEST_PASSPHRASE, document_fixture, passphrase, test_session_at};
use tauri::http::Request;
use tempfile::TempDir;

const START: &str = "2026-10-07T09:00:00+02:00";

fn pdf_bytes() -> Vec<u8> {
    document_fixture("three-pages.pdf")
}

// ------------------------------------------------------------------ fakes

type OnAsk = Box<dyn Fn(&ConsentRequest) + Send + Sync>;

/// The native dialog: records each request, answers as told (the queued
/// answers in order, then `Cancel`), and runs a hook while "the dialog is
/// up".
#[derive(Default)]
struct FakeConsent {
    requests: Mutex<Vec<ConsentRequest>>,
    answers: Mutex<VecDeque<ConsentAnswer>>,
    on_ask: Mutex<Option<OnAsk>>,
}

impl FakeConsent {
    fn answering(answers: &[ConsentAnswer]) -> Self {
        let consent = Self::default();
        consent.answers.lock().unwrap().extend(answers.iter().copied());
        consent
    }

    fn open() -> Self {
        Self::answering(&[ConsentAnswer::Open; 50])
    }

    fn cancel() -> Self {
        Self::default()
    }

    fn while_asking(&self, hook: impl Fn(&ConsentRequest) + Send + Sync + 'static) {
        *self.on_ask.lock().unwrap() = Some(Box::new(hook));
    }

    fn requests(&self) -> Vec<ConsentRequest> {
        self.requests.lock().unwrap().clone()
    }

    fn titles(&self) -> Vec<String> {
        self.requests().iter().map(ConsentRequest::title).collect()
    }
}

impl Consent for FakeConsent {
    fn ask(&self, request: ConsentRequest) -> ConsentAnswer {
        self.requests.lock().unwrap().push(request.clone());
        if let Some(hook) = self.on_ask.lock().unwrap().as_ref() {
            hook(&request);
        }
        self.answers.lock().unwrap().pop_front().unwrap_or(ConsentAnswer::Cancel)
    }
}

#[derive(Clone, Copy)]
enum Fails {
    Never,
    NoApp,
    Other,
}

/// The OS's default-app opener: records each path and the bytes the file
/// held when it was called, or fails.
struct FakeOpener {
    calls: Mutex<Vec<(PathBuf, Vec<u8>)>>,
    fails: Fails,
    on_open: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl FakeOpener {
    fn new() -> Self {
        Self { calls: Mutex::default(), fails: Fails::Never, on_open: Mutex::default() }
    }

    fn failing(fails: Fails) -> Self {
        Self { fails, ..Self::new() }
    }

    fn calls(&self) -> Vec<(PathBuf, Vec<u8>)> {
        self.calls.lock().unwrap().clone()
    }

    fn paths(&self) -> Vec<PathBuf> {
        self.calls().into_iter().map(|(path, _)| path).collect()
    }
}

impl Opener for FakeOpener {
    fn open(&self, path: &Path) -> Result<(), OpenFailure> {
        if let Some(hook) = self.on_open.lock().unwrap().as_ref() {
            hook();
        }
        let held = fs::read(path).unwrap_or_default();
        self.calls.lock().unwrap().push((path.to_owned(), held));
        match self.fails {
            Fails::Never => Ok(()),
            Fails::NoApp => Err(OpenFailure::NoApp),
            Fails::Other => Err(OpenFailure::Other("the launcher failed".into())),
        }
    }
}

/// Fails the test, rather than hangs it, if the session's lock is held by
/// the caller: the dialog and the opener run outside it (research.md §16,
/// §18).
fn assert_session_lock_free(session: &Session, who: &str) {
    let session = session.clone();
    let (done, answered) = mpsc::channel();
    thread::spawn(move || {
        let _ = session.is_open();
        let _ = done.send(());
    });
    answered
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| panic!("the session's lock was held while {who} ran"));
}

// ------------------------------------------------------------------ world

/// An open database with one firearm, on a manual clock, with the folder the
/// copies go to where the session clears it.
struct World {
    config: TempDir,
    dir: TempDir,
    session: Arc<Session>,
    machine: Arc<MachineSettings>,
    clock: Arc<ManualClock>,
    preview: TestPreview,
    firearm: i64,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let dir = TempDir::new().unwrap();
        let clock = ManualClock::at(START);
        let (session, _events) =
            test_session_at(&config.path().join("opened-documents"), clock.clone());
        let machine = Arc::new(MachineSettings::load(config.path()).unwrap());
        lifecycle::create(&session, &machine, &dir.path().join("Mine.hoplodex"), &passphrase())
            .unwrap();
        let session = Arc::new(session);
        let firearm = new_firearm(&session);
        Self { config, dir, session, machine, clock, preview: TestPreview::new(), firearm }
    }

    /// The root that holds `<document id>/<copy>`.
    fn copies(&self) -> PathBuf {
        self.config.path().join("opened-documents")
    }

    fn folder_of(&self, id: i64) -> PathBuf {
        self.copies().join(id.to_string())
    }

    fn database(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn pdf(&self, name: &str) -> i64 {
        insert_raw_document(&self.session, self.firearm, name, "application/pdf", &pdf_bytes())
    }

    fn setting(&self, setting: DocumentOpening) {
        self.machine.set_document_opening(setting);
    }

    /// `open_document`, with its answer's `opened`.
    fn open(
        &self,
        consent: &FakeConsent,
        opener: &FakeOpener,
        id: i64,
    ) -> Result<bool, CommandError> {
        document_ops::open_document(
            &self.session,
            &self.machine,
            consent,
            opener,
            &self.copies(),
            id,
        )
        .map(|answer| {
            let answer: Value = serde_json::to_value(answer).unwrap();
            answer["opened"].as_bool().unwrap_or_else(|| panic!("no `opened` in {answer}"))
        })
    }

    fn lock(&self) {
        databases::lock_database(&self.session, &self.machine, None).unwrap();
    }

    fn reopen(&self) {
        let typed = Passphrase::from_input(TEST_PASSPHRASE.to_owned());
        lifecycle::open(&self.session, &self.machine, &self.database(), &typed, false).unwrap();
    }

    fn confirmed(&self) -> bool {
        self.session.inspect(|open| Ok(open.external_open_confirmed)).unwrap()
    }

    fn generation(&self) -> u64 {
        self.session.inspect(|open| Ok(open.generation)).unwrap()
    }

    fn files(&self) -> Vec<PathBuf> {
        files_under(&self.copies())
    }
}

/// Every file (not folder) under `dir`, which may not exist.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return found };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

fn code<T: std::fmt::Debug>(result: Result<T, CommandError>) -> String {
    result.unwrap_err().code
}

fn document_request(request: &ConsentRequest) -> (&str, &str, bool) {
    match request {
        ConsentRequest::Document { name, kind, first_of_session_with_external } => {
            (name, kind, *first_of_session_with_external)
        }
        other => panic!("expected the request for a document, got {other:?}"),
    }
}

// --- The request (FR-008, US2-1, ui contract §4) --------------------------------

#[test]
fn the_default_setting_asks_about_each_open_and_names_the_document_and_its_kind() {
    let world = World::new();
    let id = world.pdf("2024 appraisal.pdf");
    let consent = FakeConsent::open();
    let opener = FakeOpener::new();

    world.open(&consent, &opener, id).unwrap();
    world.open(&consent, &opener, id).unwrap();

    let requests = consent.requests();
    assert_eq!(requests.len(), 2, "the default setting asks every time");
    for request in &requests {
        let (name, kind, first_with_external) = document_request(request);
        assert_eq!((name, kind), ("2024 appraisal.pdf", "PDF"));
        assert!(!first_with_external, "the setting isn't \"external\"");
    }
    assert_eq!(consent.titles()[0], "Open \u{201C}2024 appraisal.pdf\u{201D} in another app?");
}

#[test]
fn the_name_in_the_request_has_control_characters_replaced_and_is_cut_to_120_characters() {
    let world = World::new();
    let consent = FakeConsent::cancel();
    let opener = FakeOpener::new();
    let with_control = world.pdf("re\u{7}ceipt\u{202A}\n.pdf");
    let long_name = format!("{}.pdf", "long name ".repeat(15));
    let long = world.pdf(&long_name);

    world.open(&consent, &opener, with_control).unwrap();
    world.open(&consent, &opener, long).unwrap();

    let titles = consent.titles();
    assert!(!titles[0].chars().any(char::is_control), "{:?}", titles[0]);
    assert!(titles[0].contains("re") && titles[0].contains("ceipt"), "{:?}", titles[0]);
    let shown: String = titles[1]
        .strip_prefix("Open \u{201C}")
        .and_then(|rest| rest.strip_suffix("\u{201D} in another app?"))
        .unwrap_or_else(|| panic!("the title's shape: {:?}", titles[1]))
        .to_owned();
    assert!(shown.ends_with('\u{2026}'), "cut with an ellipsis: {shown:?}");
    assert!((119..=121).contains(&shown.chars().count()), "{} characters", shown.chars().count());
    let first_119: String = long_name.chars().take(119).collect();
    assert!(shown.starts_with(&first_119), "the start of the name is kept");
}

#[test]
fn the_rule_asks_unless_the_setting_is_external_and_this_session_has_confirmed() {
    use DocumentOpening::{External, Preview};
    assert!(needs_consent(Preview, false));
    assert!(needs_consent(Preview, true), "the session's \"yes\" counts only for \"external\"");
    assert!(needs_consent(External, false));
    assert!(!needs_consent(External, true));
}

// --- Cancel (US2-2, SC-004) -----------------------------------------------------

#[test]
fn cancel_writes_nothing_and_starts_nothing() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let consent = FakeConsent::cancel();
    let opener = FakeOpener::new();

    let opened = world.open(&consent, &opener, id).unwrap();

    assert!(!opened);
    assert_eq!(consent.requests().len(), 1);
    assert!(opener.calls().is_empty(), "the other app was not started");
    assert!(!world.copies().exists(), "no folder");
    assert!(world.files().is_empty(), "no file");
    assert!(!world.confirmed(), "a Cancel confirms nothing");
}

// --- Open (US2-3, FR-010) -------------------------------------------------------

#[test]
fn open_writes_a_copy_named_by_the_stored_stem_and_the_canonical_extension() {
    let world = World::new();
    let id = world.pdf("receipt.Pdf");
    let consent = FakeConsent::open();
    let opener = FakeOpener::new();
    opener.on_open.lock().unwrap().replace({
        let session = world.session.clone();
        Box::new(move || assert_session_lock_free(&session, "the opener"))
    });

    let opened = world.open(&consent, &opener, id).unwrap();

    assert!(opened);
    let expected = world.folder_of(id).join("receipt.pdf");
    assert_eq!(opener.paths(), vec![expected.clone()], "the opener is called once, with the copy");
    assert_eq!(opener.calls()[0].1, pdf_bytes(), "the file was complete when the opener got it");
    assert_eq!(world.files(), vec![expected], "the canonical extension, not \"Pdf\"");
    assert!(world.confirmed(), "an \"Open in another app\" confirms this session (FR-012)");
}

#[test]
fn a_name_with_path_parts_stays_in_its_own_folder() {
    let world = World::new();
    let id = world.pdf("..\\..\\up/evil:name?.pdf");
    let consent = FakeConsent::open();
    let opener = FakeOpener::new();

    world.open(&consent, &opener, id).unwrap();

    let files = world.files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].parent().unwrap(), world.folder_of(id), "{files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.ends_with(".pdf") && !name.contains([':', '?', '/', '\\']), "{name}");
}

#[test]
fn a_second_open_reuses_a_same_length_copy_and_rewrites_a_damaged_one() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let consent = FakeConsent::open();
    let opener = FakeOpener::new();
    world.open(&consent, &opener, id).unwrap();
    let copy = world.folder_of(id).join("receipt.pdf");
    let long_ago = SystemTime::now() - Duration::from_secs(3600);
    fs::OpenOptions::new().write(true).open(&copy).unwrap().set_modified(long_ago).unwrap();

    world.open(&consent, &opener, id).unwrap();

    assert_eq!(opener.paths(), vec![copy.clone(), copy.clone()], "opened again, at the same path");
    let modified = fs::metadata(&copy).unwrap().modified().unwrap();
    assert!(modified <= long_ago + Duration::from_secs(2), "a same-length copy isn't rewritten");

    fs::write(&copy, b"cut short").unwrap();
    world.open(&consent, &opener, id).unwrap();

    assert_eq!(fs::read(&copy).unwrap(), pdf_bytes(), "a copy of another length is rewritten");
    assert_eq!(opener.calls()[2].1, pdf_bytes());
}

#[test]
fn the_copy_is_cleared_on_close_and_by_the_startup_sweep() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let consent = FakeConsent::open();
    let opener = FakeOpener::new();
    world.open(&consent, &opener, id).unwrap();
    assert_eq!(world.files().len(), 1);

    lifecycle::close_normal(&world.session, &world.machine, CloseReason::Closed).unwrap();

    assert!(world.files().is_empty(), "closing the database deletes the copy");

    world.reopen();
    world.open(&consent, &opener, id).unwrap();
    assert_eq!(world.files().len(), 1);
    // A crash left it: the next start sweeps the folder (001 FR-035).
    let left = document_ops::clear_opened_documents(&world.copies());

    assert!(left.is_empty());
    assert!(world.files().is_empty(), "the startup sweep deletes it");
}

// --- The session's confirmation (FR-012, research.md §17) ----------------------

#[test]
fn with_the_setting_external_one_yes_covers_the_session_and_a_lock_forgets_it() {
    let world = World::new();
    world.setting(DocumentOpening::External);
    let id = world.pdf("receipt.pdf");
    let consent =
        FakeConsent::answering(&[ConsentAnswer::Cancel, ConsentAnswer::Open, ConsentAnswer::Open]);
    let opener = FakeOpener::new();

    assert!(!world.open(&consent, &opener, id).unwrap(), "a Cancel is not a yes");
    assert!(!world.confirmed());
    assert!(world.open(&consent, &opener, id).unwrap());
    assert!(world.confirmed());
    assert_eq!(consent.requests().len(), 2, "so far each open asked");
    assert!(world.open(&consent, &opener, id).unwrap(), "no dialog this time");
    assert_eq!(consent.requests().len(), 2, "a confirmed session isn't asked again");
    assert_eq!(opener.calls().len(), 2);

    world.lock();
    world.reopen();

    assert!(!world.confirmed(), "a lock forgets it");
    assert!(world.open(&consent, &opener, id).unwrap());
    assert_eq!(consent.requests().len(), 3, "the next session asks again");
}

#[test]
fn each_open_or_unlock_is_a_new_generation() {
    let world = World::new();
    let first = world.generation();

    world.lock();
    world.reopen();

    assert_ne!(world.generation(), first);
}

// --- A lock while the dialog is up (#65, US2-6) ---------------------------------

#[test]
fn issue_65_a_lock_while_the_dialog_is_up_writes_no_copy_and_starts_nothing() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let consent = FakeConsent::open();
    consent.while_asking({
        let session = world.session.clone();
        let machine = world.machine.clone();
        move |_| {
            assert_session_lock_free(&session, "the dialog");
            databases::lock_database(&session, &machine, None).unwrap();
        }
    });
    let opener = FakeOpener::new();

    let refused = world.open(&consent, &opener, id);

    assert_eq!(code(refused), "DATABASE_CLOSED");
    assert!(opener.calls().is_empty());
    assert!(!world.copies().exists(), "nothing was written");
}

#[test]
fn issue_65_a_copy_is_written_only_into_the_session_that_asked() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let consent = FakeConsent::open();
    // The same database is locked and unlocked while the dialog is up: it is
    // open again, but it is not the session that asked.
    consent.while_asking({
        let session = world.session.clone();
        let machine = world.machine.clone();
        let path = world.database();
        move |_| {
            databases::lock_database(&session, &machine, None).unwrap();
            let typed = Passphrase::from_input(TEST_PASSPHRASE.to_owned());
            lifecycle::open(&session, &machine, &path, &typed, false).unwrap();
        }
    });
    let opener = FakeOpener::new();
    let generation = world.generation();

    let refused = world.open(&consent, &opener, id);

    assert_eq!(code(refused), "DATABASE_CLOSED");
    assert_ne!(world.generation(), generation, "the session was replaced");
    assert!(opener.calls().is_empty());
    assert!(!world.copies().exists(), "nothing was written for the new session");
    assert!(!world.confirmed(), "the old session's \"yes\" didn't carry over");
}

#[test]
fn issue_65_a_lock_racing_the_write_clears_the_copy_or_never_writes_it() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let consent = FakeConsent::open();
    let opener = FakeOpener::new();
    let mut written = 0;
    let mut refused = 0;

    for round in 0..16u64 {
        let locker = {
            let session = world.session.clone();
            let machine = world.machine.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_micros(round * 400));
                databases::lock_database(&session, &machine, None).unwrap();
            })
        };
        match world.open(&consent, &opener, id) {
            Ok(opened) => {
                assert!(opened);
                written += 1;
            }
            Err(error) => {
                assert_eq!(error.code, "DATABASE_CLOSED");
                refused += 1;
            }
        }
        locker.join().unwrap();

        assert!(world.files().is_empty(), "round {round}: a copy outlived the lock");
        world.reopen();
    }

    assert_eq!(opener.calls().len(), written, "the opener ran for each open that succeeded");
    assert_eq!(written + refused, 16);
}

// --- Private from creation (#71) -------------------------------------------------

/// Modes seen on anything under `dir` by a thread that polls it while the
/// copy is written: a folder or file that is ever wider than it should be.
#[cfg(unix)]
struct ModeWatch {
    stop: Arc<AtomicBool>,
    seen: thread::JoinHandle<Vec<(PathBuf, bool, u32)>>,
}

#[cfg(unix)]
impl ModeWatch {
    fn start(dir: PathBuf) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let seen = thread::spawn(move || {
            let mut seen = Vec::new();
            let walk = |path: &Path, seen: &mut Vec<(PathBuf, bool, u32)>| {
                if let Ok(meta) = fs::symlink_metadata(path) {
                    seen.push((path.to_owned(), meta.is_dir(), meta.permissions().mode() & 0o7777));
                }
            };
            while !flag.load(Ordering::Relaxed) {
                walk(&dir, &mut seen);
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        walk(&entry.path(), &mut seen);
                        if let Ok(inner) = fs::read_dir(entry.path()) {
                            for file in inner.flatten() {
                                walk(&file.path(), &mut seen);
                            }
                        }
                    }
                }
            }
            seen
        });
        Self { stop, seen }
    }

    fn finish(self) -> Vec<(PathBuf, bool, u32)> {
        self.stop.store(true, Ordering::Relaxed);
        self.seen.join().unwrap()
    }
}

#[cfg(unix)]
#[test]
fn issue_71_the_folders_are_0700_and_the_copy_0600_from_creation() {
    use std::os::unix::fs::PermissionsExt;

    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let watch = ModeWatch::start(world.copies());
    // A permissive umask must not widen anything: the modes are asked for at
    // creation, not left to the umask or fixed after.
    let before = unsafe { libc::umask(0) };

    let opened = world.open(&FakeConsent::open(), &FakeOpener::new(), id);

    unsafe { libc::umask(before) };
    let seen = watch.finish();
    assert!(opened.unwrap());
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode(&world.copies()), 0o700, "the opened-documents folder");
    assert_eq!(mode(&world.folder_of(id)), 0o700, "the document's folder");
    assert_eq!(mode(&world.folder_of(id).join("receipt.pdf")), 0o600, "the copy");
    for (path, is_dir, seen_mode) in seen {
        let allowed = if is_dir { 0o700 } else { 0o600 };
        assert_eq!(seen_mode, allowed, "{} was {seen_mode:o} when seen", path.display());
    }
}

#[cfg(unix)]
#[test]
fn issue_71_a_symlinked_document_folder_is_refused_and_nothing_is_written_through_it() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let elsewhere = TempDir::new().unwrap();
    fs::create_dir_all(world.copies()).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), world.folder_of(id)).unwrap();
    let opener = FakeOpener::new();

    let refused = world.open(&FakeConsent::open(), &opener, id);

    assert_eq!(code(refused), "INTERNAL_ERROR");
    assert!(opener.calls().is_empty());
    assert!(files_under(elsewhere.path()).is_empty(), "nothing went through the link");
}

#[cfg(unix)]
#[test]
fn issue_71_a_folder_owned_by_someone_else_is_refused() {
    // Needs the right to give a folder away, so only as root (the dev
    // container's user is not). Elsewhere the check is the same call as the
    // symlink case's, and this one reports itself skipped.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: not running as root, so a folder can't be given to another user");
        return;
    }
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    fs::create_dir_all(world.folder_of(id)).unwrap();
    std::os::unix::fs::chown(world.folder_of(id), Some(65534), Some(65534)).unwrap();
    let opener = FakeOpener::new();

    let refused = world.open(&FakeConsent::open(), &opener, id);

    assert_eq!(code(refused), "INTERNAL_ERROR");
    assert!(opener.calls().is_empty());
    assert!(world.files().is_empty(), "no file in a folder someone else owns");
}

// --- No app for the type (US2-5) -------------------------------------------------

#[test]
fn an_opener_with_no_app_for_the_type_deletes_the_copy_at_once() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let opener = FakeOpener::failing(Fails::NoApp);

    let refused = world.open(&FakeConsent::open(), &opener, id);

    assert_eq!(code(refused), "NO_APP_FOR_DOCUMENT");
    assert_eq!(opener.calls().len(), 1, "it was tried");
    assert_eq!(opener.calls()[0].1, pdf_bytes(), "with a complete copy");
    assert!(world.files().is_empty(), "the copy is gone");
}

#[test]
fn any_other_opener_failure_is_an_internal_error() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");

    let refused = world.open(&FakeConsent::open(), &FakeOpener::failing(Fails::Other), id);

    assert_eq!(code(refused), "INTERNAL_ERROR");
}

// --- Refusals before any dialog (US2-7, FR-017) ---------------------------------

#[test]
fn a_row_the_attach_rules_would_refuse_is_refused_before_the_dialog_and_can_be_deleted() {
    let world = World::new();
    let photo = hostile::jpg();
    let html = hostile::html_named_pdf();
    let jpeg_row =
        insert_raw_document(&world.session, world.firearm, &photo.name, "image/jpeg", &photo.bytes);
    let html_row = insert_raw_document(
        &world.session,
        world.firearm,
        &html.name,
        "application/pdf",
        &html.bytes,
    );
    let consent = FakeConsent::open();
    let opener = FakeOpener::new();

    assert_eq!(code(world.open(&consent, &opener, jpeg_row)), "DOCUMENT_TYPE_NOT_ALLOWED");
    assert_eq!(code(world.open(&consent, &opener, html_row)), "DOCUMENT_CONTENT_MISMATCH");

    assert!(consent.requests().is_empty(), "the dialog was never shown");
    assert!(opener.calls().is_empty());
    assert!(!world.copies().exists());
    for id in [jpeg_row, html_row] {
        world.session.write(|conn| document_ops::delete_document(conn, id, true)).unwrap();
    }
}

#[test]
fn a_missing_document_is_not_found_before_any_dialog() {
    let world = World::new();
    let consent = FakeConsent::open();

    let refused = world.open(&consent, &FakeOpener::new(), 9_999);

    assert_eq!(code(refused), "NOT_FOUND");
    assert!(consent.requests().is_empty());
}

// --- The idle clock and the PDF surface while the dialog is up -------------------

#[test]
fn the_idle_clock_is_paused_while_the_dialog_is_up_and_starts_again_after() {
    let world = World::new();
    databases::update_lock_settings(
        &world.session,
        &LockSettingsInput { idle_enabled: true, idle_minutes: 10, on_screen_lock: false },
    )
    .unwrap();
    let id = world.pdf("receipt.pdf");
    let consent = FakeConsent::open();
    consent.while_asking({
        let session = world.session.clone();
        let machine = world.machine.clone();
        let clock = world.clock.clone();
        move |_| {
            clock.advance(chrono::Duration::minutes(30));
            assert!(!lifecycle::idle_tick(&session, &machine), "paused: no lock in the dialog");
        }
    });

    world.open(&consent, &FakeOpener::new(), id).unwrap();

    assert!(world.session.is_open());
    assert!(!lifecycle::idle_tick(&world.session, &world.machine), "the idle time began again");
    world.clock.advance(chrono::Duration::minutes(9));
    assert!(!lifecycle::idle_tick(&world.session, &world.machine));
    world.clock.advance(chrono::Duration::minutes(1));
    assert!(lifecycle::idle_tick(&world.session, &world.machine), "and runs out as usual");
}

#[test]
fn a_shown_pdf_surface_is_hidden_while_the_dialog_is_up() {
    let world = World::new();
    let id = add_document(&world.session, world.firearm, "receipt.pdf", &pdf_bytes());
    let info = serde_json::to_value(
        preview::open_preview(&world.session, &world.preview.env, id).unwrap(),
    )
    .unwrap();
    let preview_id = info["previewId"].as_u64().unwrap();
    let surface = world.preview.surfaces.only();
    // Serve it, and on Linux report PDF.js's hook, so that it can be shown.
    let url = surface.url();
    let get = |url: &str| {
        let request = Request::builder().method("GET").uri(url).body(Vec::new()).unwrap();
        protocol_handler::handle(&world.session, &request)
    };
    assert_eq!(get(&url).status().as_u16(), 200);
    if cfg!(target_os = "linux") {
        let mut hooked = tauri::Url::parse(&url).unwrap();
        hooked.set_path(&format!("/hooked/{}", surface.secret()));
        assert_eq!(get(hooked.as_str()).status().as_u16(), 204);
    }
    let bounds = SurfaceBounds { x: 10.0, y: 10.0, width: 400.0, height: 300.0 };
    preview::set_preview_bounds(&world.session, preview_id, bounds, true).unwrap();
    assert!(surface.bounds_calls().last().unwrap().1, "it is shown before the dialog");
    let consent = FakeConsent::cancel();
    let hidden_while_asking = Arc::new(AtomicBool::new(false));
    consent.while_asking({
        let surface = surface.clone();
        let hidden = hidden_while_asking.clone();
        move |_| {
            let last = surface.bounds_calls().last().map(|(_, visible)| *visible);
            hidden.store(last == Some(false), Ordering::Relaxed);
        }
    });

    world.open(&consent, &FakeOpener::new(), id).unwrap();

    assert!(hidden_while_asking.load(Ordering::Relaxed), "hidden while the dialog was up");
    assert!(!surface.is_closed(), "hidden, not closed: the viewer stays");
}

// --- The text (ui contract §4) ----------------------------------------------------

/// `needles` all appear in `text`, in this order.
fn assert_in_order(text: &str, needles: &[&str]) {
    let mut from = 0;
    for needle in needles {
        match text[from..].find(needle) {
            Some(at) => from += at + needle.len(),
            None => panic!("{needle:?} is missing, or out of order, in:\n{text}"),
        }
    }
}

const FOUR_CONSEQUENCES: [&str; 4] = [
    "Other apps, and anyone who can use this computer account, can read the copy while it is there.",
    "The other app may keep its own copies or records of the document, such as recent-files \
     lists, autosaves, caches or cloud sync. HoploDex can't find or delete those.",
    "The other app may connect to the internet if the document asks it to, for example to load \
     a picture or follow a link, which can show that the document was opened.",
    "HoploDex deletes its copy when this database is closed or locked or HoploDex quits, so the \
     other app may lose the document then. Changes saved to the copy are not kept in your \
     collection.",
];

#[test]
fn the_text_for_a_document_is_the_contracts_word_for_word() {
    let request = ConsentRequest::Document {
        name: "Bill of sale.docx".into(),
        kind: "Word".into(),
        first_of_session_with_external: false,
    };

    assert_eq!(request.title(), "Open \u{201C}Bill of sale.docx\u{201D} in another app?");
    let body = request.body();
    assert_in_order(
        &body,
        &[
            "HoploDex will put an unprotected copy of this document on this computer and open it \
             in the app this computer uses for Word documents.",
            FOUR_CONSEQUENCES[0],
            FOUR_CONSEQUENCES[1],
            FOUR_CONSEQUENCES[2],
            FOUR_CONSEQUENCES[3],
        ],
    );
    assert!(!body.contains("You won't be asked again"), "only the session's first external open");

    let first = ConsentRequest::Document {
        name: "Bill of sale.docx".into(),
        kind: "Word".into(),
        first_of_session_with_external: true,
    };
    assert_in_order(
        &first.body(),
        &[
            FOUR_CONSEQUENCES[3],
            "You won't be asked again until this database is closed or locked.",
        ],
    );
}

#[test]
fn the_text_for_the_setting_is_the_contracts_word_for_word() {
    let request = ConsentRequest::Setting;

    assert_eq!(request.title(), "Open documents in another app?");
    assert_in_order(
        &request.body(),
        &[
            "Each document you open will be copied, unprotected, to this computer and opened in \
             the app this computer uses for its type. This applies to every database on this \
             computer.",
            FOUR_CONSEQUENCES[0],
            FOUR_CONSEQUENCES[1],
            FOUR_CONSEQUENCES[2],
            FOUR_CONSEQUENCES[3],
            "You'll be asked once each time a database is opened or unlocked.",
        ],
    );
}

// --- Per-OS cases ------------------------------------------------------------------
//
// T091 adds here, `#[cfg(windows)]`: the per-document folder's DACL grants
// only the current user and is protected, and the copy has a
// `:Zone.Identifier` stream holding `ZoneId=3`, read back.

/// The copy's `com.apple.quarantine` attribute, read with `getxattr`, or
/// `None` if it has none.
#[cfg(target_os = "macos")]
fn quarantine_of(path: &Path) -> Option<String> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    let mut value = vec![0u8; 256];
    // SAFETY: both names are NUL-terminated and `value` is writable for its
    // length.
    let read = unsafe {
        libc::getxattr(
            path.as_ptr(),
            c"com.apple.quarantine".as_ptr(),
            value.as_mut_ptr().cast(),
            value.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if read < 0 {
        let error = std::io::Error::last_os_error();
        assert_eq!(error.raw_os_error(), Some(libc::ENOATTR), "getxattr: {error}");
        return None;
    }
    value.truncate(read as usize);
    Some(String::from_utf8(value).unwrap())
}

/// T090 (research.md §18): the copy handed to the other app carries a
/// quarantine mark in the form a browser gives a download, and keeps it when
/// a second open reuses the copy.
#[cfg(target_os = "macos")]
#[test]
fn macos_the_copy_carries_a_quarantine_attribute() {
    let world = World::new();
    let id = world.pdf("receipt.pdf");
    let opener = FakeOpener::new();
    let before = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();

    assert!(world.open(&FakeConsent::open(), &opener, id).unwrap());

    let copy = world.folder_of(id).join("receipt.pdf");
    assert_eq!(opener.paths(), vec![copy.clone()]);
    let mark = quarantine_of(&copy).expect("the copy has no com.apple.quarantine attribute");
    let fields: Vec<&str> = mark.split(';').collect();
    assert_eq!(fields.len(), 4, "flags;time;agent;event, got {mark:?}");
    assert_eq!(fields[0], "0081", "the flags in {mark:?}");
    let marked_at = u64::from_str_radix(fields[1], 16).unwrap();
    assert!(marked_at >= before && marked_at <= before + 60, "the time in {mark:?}");
    assert_eq!(fields[2], "HoploDex", "the agent in {mark:?}");

    // A second open reuses the copy, which is still marked.
    assert!(world.open(&FakeConsent::open(), &opener, id).unwrap());
    assert_eq!(opener.paths(), vec![copy.clone(), copy.clone()]);
    assert_eq!(quarantine_of(&copy).as_deref(), Some(mark.as_str()));
}
