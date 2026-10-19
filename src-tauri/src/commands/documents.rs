use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, named_params};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::app_dirs;
use crate::commands::CommandError;
use crate::commands::firearms::DeleteResult;
use crate::models::database::IdlePauseReason;
use crate::models::document_attachment::{DocumentAttachment, DocumentSummary};
use crate::models::document_opening::DocumentOpening;
use crate::models::record::RecordRef;
use crate::models::rules::MAX_DOCUMENTS_PER_RECORD;
use crate::services::attachments::{read_attachment_file, require_room, safe_basename};
use crate::services::consent::{Consent, ConsentAnswer, ConsentRequest, needs_consent};
use crate::services::document_types::{self, DocumentType, classify};
use crate::services::machine_settings::MachineSettings;
use crate::services::preview::availability::PdfAvailabilityState;
use crate::services::sandbox::{Need, Reach};
use crate::services::secure_delete::{secure_delete_dir, secure_delete_file};
use crate::session::scoped::ScopedSession;
use crate::session::{Session, SessionId};

/// Why the OS could not open a copy.
#[derive(Debug)]
pub enum OpenFailure {
    /// The OS has no program for the file's type (`SE_ERR_NOASSOC`,
    /// `open`'s non-zero exit, `xdg-open`'s exit 3 or 4; 007 research.md §18).
    NoApp,
    Other(String),
}

/// Hands a file to the OS's default program for its type: the app's on the
/// platform's own launcher, the E2E build's on a log, the tests' on a fake.
/// What it starts is the other app's, not HoploDex's: it is never waited for
/// past its start, and it outlives HoploDex.
pub trait Opener: Send + Sync {
    /// Whether the OS has a program for files with this extension (no dot,
    /// the type's canonical one), asked before any dialog or copy
    /// (007 research.md §18, amended 2026-10-07). The default is yes: Linux and
    /// macOS learn there is none from `open`'s exit status, and Windows
    /// answers from its file associations, because `ShellExecuteExW` shows
    /// its "How do you want to open this file?" picker rather than failing.
    fn has_app(&self, _extension: &str) -> bool {
        true
    }

    fn open(&self, path: &Path) -> Result<(), OpenFailure>;
}

/// What `open_document` answers: `opened` is false when the user cancelled
/// the native confirmation.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct OpenedDocument {
    pub opened: bool,
}

/// Pure, `Connection`-based business logic — mirrors `commands::firearms::ops`
/// (constitution: no mocks, integration tests call these directly against a
/// real temporary SQLCipher database).
pub mod ops {
    use super::*;

    pub fn get_document(conn: &Connection, id: i64) -> Result<DocumentAttachment, CommandError> {
        conn.query_row(
            "SELECT * FROM document_attachments WHERE id = :id",
            named_params! { ":id": id },
            DocumentAttachment::from_row,
        )
        .optional()
        .map_err(CommandError::from_db)?
        .ok_or_else(|| CommandError::not_found("No document was found with that id."))
    }

    pub fn list_documents(
        conn: &Connection,
        owner: RecordRef,
    ) -> Result<Vec<DocumentAttachment>, CommandError> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT * FROM document_attachments WHERE {} = :id ORDER BY created_at, id",
                owner.owner_column()
            ))
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(named_params! { ":id": owner.id() }, DocumentAttachment::from_row)
            .map_err(CommandError::from_db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::from_db)
    }

    /// The document types in table order (007 contracts/tauri-commands.md
    /// `list_document_types`): what the picker's `accept` and the drop
    /// router read, so the frontend keeps no list of its own.
    pub fn list_document_types() -> &'static [DocumentType] {
        document_types::all()
    }

    pub fn add_document(
        conn: &Connection,
        owner: RecordRef,
        file_bytes: &[u8],
        original_filename: &str,
    ) -> Result<DocumentAttachment, CommandError> {
        // The type recorded is the one the content check finds, whatever the
        // file chooser said (007 FR-016). Nothing is stored on a refusal.
        let document_type = classify(original_filename, file_bytes)?;
        // The name comes over IPC. A name is only ever a name: no path in it
        // (issue #68; the copy for another app uses `safe_stem` as well).
        let original_filename = safe_basename(original_filename, "document");
        let exists: bool = conn
            .query_row(
                &format!("SELECT EXISTS (SELECT 1 FROM {} WHERE id = :id)", owner.table()),
                named_params! { ":id": owner.id() },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        if !exists {
            return Err(CommandError::not_found(format!(
                "No {} was found with that id.",
                owner.noun()
            )));
        }
        // At most `MAX_DOCUMENTS_PER_RECORD` (001 FR-010, amended 2026-10-11).
        require_room(conn, owner, "document_attachments", "documents", MAX_DOCUMENTS_PER_RECORD)?;
        let (firearm_id, accessory_id) = owner.owner_columns();
        conn.execute(
            "INSERT INTO document_attachments (
                uid, firearm_id, accessory_id, file_bytes, original_filename, mime_type, created_at
            ) VALUES (
                :uid, :firearm_id, :accessory_id, :file_bytes, :original_filename, :mime_type,
                datetime('now')
            )",
            named_params! {
                // 009 FR-001: the record's identifier, never sent over IPC.
                ":uid": crate::services::record_id::generate(),
                ":firearm_id": firearm_id,
                ":accessory_id": accessory_id,
                ":file_bytes": file_bytes,
                ":original_filename": original_filename,
                ":mime_type": document_type.mime_type,
            },
        )
        .map_err(CommandError::from_db)?;

        get_document(conn, conn.last_insert_rowid())
    }

    /// `add_document` for a file already on disk — a document dropped onto
    /// the window arrives as a path, not as bytes. It is classified by its
    /// content like any other.
    ///
    /// Inside a Flatpak a file that can't be seen and lies outside reach is
    /// refused with `LOCATION_OUT_OF_REACH` before it is read, for the file
    /// chooser and a drop alike (011 FR-007, 011 US1-5).
    pub fn add_document_from_path(
        conn: &Connection,
        owner: RecordRef,
        path: &Path,
        reach: &Reach,
    ) -> Result<DocumentAttachment, CommandError> {
        let path = reach.check(path, Need::ReadFile)?;
        let file = read_attachment_file(&path)?;
        add_document(conn, owner, &file.bytes, &file.filename)
    }

    pub fn delete_document(
        conn: &Connection,
        id: i64,
        confirmed: bool,
    ) -> Result<DeleteResult, CommandError> {
        if !confirmed {
            return Err(CommandError::new(
                "CONFIRMATION_REQUIRED",
                "Deletion must be explicitly confirmed.",
            ));
        }
        let deleted = conn
            .execute("DELETE FROM document_attachments WHERE id = :id", named_params! { ":id": id })
            .map_err(CommandError::from_db)?;
        if deleted == 0 {
            return Err(CommandError::not_found("No document was found with that id."));
        }
        // The file name is in the document-names index (007 research.md §19).
        crate::db::reclaim_deleted_record(conn);
        Ok(DeleteResult { deleted: true })
    }

    /// Reopens a document from its record in the OS's default program for
    /// its type, through a temporary copy under `opened_documents_dir`, after
    /// the user's native confirmation (007 FR-008 to FR-012,
    /// contracts/tauri-commands.md "open_document"). The dialog and the opener run
    /// outside the session's lock; the copy is written under it, and only
    /// into the open that asked (007 research.md §18).
    pub fn open_document(
        session: &Session,
        session_id: SessionId,
        machine: &MachineSettings,
        consent: &dyn Consent,
        opener: &dyn Opener,
        opened_documents_dir: &Path,
        id: i64,
    ) -> Result<OpenedDocument, CommandError> {
        // 1. The document, and the open it was read from.
        let (document, stamp) = session.read_stamped(session_id, |conn| get_document(conn, id))?;
        // 2. Refused by the attach rules before any dialog (007 FR-017).
        let document_type = classify(&document.original_filename, &document.file_bytes)?;
        // 2a. No program for the type: say so before asking or writing anything
        // (007 FR-010, research.md §18).
        if !opener.has_app(document_type.canonical_extension) {
            return Err(no_app_error(document_type, false));
        }
        // 3. Ask, unless this session has already said yes to the setting.
        let setting = machine.document_opening();
        let mut said_yes = false;
        if needs_consent(setting, stamp.external_open_confirmed) {
            crate::commands::preview::ops::hide_pdf_surface(session, session_id);
            let request = ConsentRequest::Document {
                name: document.original_filename.clone(),
                kind: document_type.label.to_owned(),
                first_of_session_with_external: setting == DocumentOpening::External
                    && !stamp.external_open_confirmed,
            };
            let answer = {
                let _paused = IdlePause::new(session);
                consent.ask(request)
            };
            if answer == ConsentAnswer::Cancel {
                return Ok(OpenedDocument { opened: false });
            }
            said_yes = true;
        }
        // 4. The copy, under the lock, into the open that asked.
        let path = session.inspect_mut(session_id, |open| {
            if said_yes {
                open.external_open_confirmed = true;
            }
            write_copy(opened_documents_dir, id, &document, document_type)
        })?;
        // 5. The lock is free: start the other app.
        match opener.open(&path) {
            Ok(()) => Ok(OpenedDocument { opened: true }),
            Err(OpenFailure::NoApp) => {
                delete_copy(&path);
                Err(no_app_error(document_type, true))
            }
            Err(OpenFailure::Other(reason)) => Err(CommandError::new(
                "INTERNAL_ERROR",
                format!("Could not open the document: {reason}"),
            )),
        }
    }

    /// `NO_APP_FOR_DOCUMENT`; the second sentence only when a copy had been
    /// made and was deleted.
    fn no_app_error(document_type: &DocumentType, copy_deleted: bool) -> CommandError {
        let mut message =
            format!("This computer has no app that opens {} documents.", document_type.label);
        if copy_deleted {
            message.push_str(" HoploDex deleted the copy it made.");
        }
        CommandError::new("NO_APP_FOR_DOCUMENT", message)
    }

    /// The native dialog is up: the idle clock waits for it, as it does for
    /// a file chooser (007 research.md §16), and starts again when this is
    /// dropped.
    pub(crate) struct IdlePause<'a>(&'a Session);

    impl<'a> IdlePause<'a> {
        pub(crate) fn new(session: &'a Session) -> Self {
            session.idle().set_paused(IdlePauseReason::NativeDialog, true, session.clock().now());
            Self(session)
        }
    }

    impl Drop for IdlePause<'_> {
        fn drop(&mut self) {
            self.0.idle().set_paused(IdlePauseReason::NativeDialog, false, self.0.clock().now());
        }
    }

    /// Securely deletes a copy `open_document` made and its folder, once the
    /// OS had no program for it.
    fn delete_copy(path: &Path) {
        if let Err(err) = secure_delete_file(path) {
            log::warn!("could not delete the opened-document copy {}: {err}", path.display());
        }
        if let Some(folder) = path.parent() {
            let _ = std::fs::remove_dir(folder);
        }
    }

    /// Securely deletes every temporary copy `open_document` left under
    /// `dir` (001 FR-035), returning the files that could not be deleted. Run on
    /// normal exit, and again at every startup as the backstop for crashes,
    /// forced kills, and anything a previous run failed to delete.
    pub fn clear_opened_documents(dir: &Path) -> Vec<PathBuf> {
        secure_delete_dir(dir)
    }
}

/// Folder under the app cache directory that holds the temporary copies
/// `open_document` hands to the OS. Cleared on exit and again at startup
/// (001 FR-035) so decrypted copies never outlive the session that made them.
pub const OPENED_DOCUMENTS_DIR: &str = "opened-documents";

/// Clears [`OPENED_DOCUMENTS_DIR`] under the app cache directory, logging
/// (never failing on) anything that couldn't be deleted — the next startup
/// sweep retries it.
pub fn clear_opened_documents_cache(app: &AppHandle) {
    let Ok(cache_dir) = app_dirs::cache_dir(app) else {
        return;
    };
    for path in ops::clear_opened_documents(&cache_dir.join(OPENED_DOCUMENTS_DIR)) {
        log::warn!("could not delete opened-document copy {}", path.display());
    }
}

/// Reopens a document from its record (001 FR-010) in the OS default app for
/// its file type, via a temporary copy under [`OPENED_DOCUMENTS_DIR`], after
/// the native confirmation. There is no flag the web view could set to
/// confirm: the answer is the user's, in a dialog Rust shows.
#[tauri::command]
pub async fn open_document(
    id: i64,
    app: AppHandle,
    session: ScopedSession,
) -> Result<OpenedDocument, CommandError> {
    let dir = app_dirs::cache_dir(&app)
        .map_err(|e| CommandError::new("INTERNAL_ERROR", e.to_string()))?
        .join(OPENED_DOCUMENTS_DIR);
    // The dialog blocks until it is answered, so not on an async thread.
    tauri::async_runtime::spawn_blocking(move || {
        let machine = app.state::<MachineSettings>();
        let consent = app.state::<Arc<dyn Consent>>();
        let opener = app.state::<Arc<dyn Opener>>();
        ops::open_document(&session.session, session.id, &machine, &**consent, &**opener, &dir, id)
    })
    .await
    .map_err(|e| CommandError::new("INTERNAL_ERROR", format!("Could not open the document: {e}")))?
}

/// How the app's [`Opener`] hands a copy on, chosen once at startup from
/// `Reach::is_flatpak()` (011 research.md §8).
#[cfg(app_opener)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenerRoute {
    /// The platform's own launcher: `xdg-open` on Linux, `open` on macOS, the
    /// shell on Windows.
    XdgOpen,
    /// Inside a Flatpak: the OpenURI portal, which says whether an application
    /// took the file. No answer within `timeout` counts as opened.
    #[cfg(target_os = "linux")]
    Portal { timeout: std::time::Duration },
}

/// How long the portal route waits for an answer before it counts the document
/// as opened (a chooser may be waiting on the user).
#[cfg(all(target_os = "linux", app_opener))]
const PORTAL_ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The route this run hands a document on by: inside a Flatpak the OpenURI
/// portal, which says whether an application took it, and anywhere else the
/// platform's own launcher (011 research.md §8, 011 FR-008).
#[cfg(app_opener)]
pub fn opener_route(reach: &Reach) -> OpenerRoute {
    #[cfg(target_os = "linux")]
    if reach.is_flatpak() {
        return OpenerRoute::Portal { timeout: PORTAL_ANSWER_TIMEOUT };
    }
    #[cfg(not(target_os = "linux"))]
    let _ = reach;
    OpenerRoute::XdgOpen
}

/// The app's [`Opener`]: the platform's own launcher, or inside a Flatpak the
/// OpenURI portal, detached either way, so what it starts is not tied to
/// HoploDex's lifetime.
#[cfg(app_opener)]
pub struct AppOpener<R: tauri::Runtime = tauri::Wry> {
    // Where the platform has no launcher of its own to run (Windows).
    #[cfg_attr(unix, allow(dead_code))]
    app: tauri::AppHandle<R>,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    route: OpenerRoute,
    /// The session-bus connection of the portal route, made at the first open.
    #[cfg(target_os = "linux")]
    portal_connection: std::sync::Mutex<Option<zbus::Connection>>,
}

#[cfg(app_opener)]
impl<R: tauri::Runtime> AppOpener<R> {
    pub fn new(app: tauri::AppHandle<R>, route: OpenerRoute) -> Self {
        Self {
            app,
            route,
            #[cfg(target_os = "linux")]
            portal_connection: std::sync::Mutex::new(None),
        }
    }

    /// The portal route: `None` when there is no session bus, which is the
    /// same answer as no portal on it.
    #[cfg(target_os = "linux")]
    fn portal(&self) -> Option<zbus::Connection> {
        let mut slot = self.portal_connection.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            *slot = zbus::block_on(zbus::Connection::session())
                .map_err(|err| log::warn!("no session bus for the OpenURI portal: {err}"))
                .ok();
        }
        slot.clone()
    }
}

/// The OpenURI portal's route (011 research.md §8; 011 data-model.md "The
/// portal opener"): `OpenFile` with a read-only descriptor for `copy` and
/// `writable: false`, then the request's `Response`, which is what says whether
/// an application took it. Blocks until the answer, `timeout` or the error, as
/// [`Opener::open`] does.
///
/// | answer | result |
/// |---|---|
/// | `Response` 0, or 1 (the user closed the portal's chooser) | `Ok(())` |
/// | `Response` 2 | `Err(NoApp)` |
/// | none within `timeout` (the chooser may be waiting on the user) | `Ok(())` |
/// | no portal on the bus, or the call refused | `Err(NoApp)` |
///
/// The portal may send `Response` before the call returns, so the request
/// object's path is known beforehand (from `handle_token`) and the signal is
/// subscribed to first.
#[cfg(target_os = "linux")]
pub fn portal_open(
    connection: &zbus::Connection,
    copy: &Path,
    timeout: std::time::Duration,
) -> Result<(), OpenFailure> {
    use std::collections::HashMap;
    use std::sync::mpsc;

    use zbus::blocking::{Connection as Blocking, MessageIterator, Proxy};
    use zbus::message::Type;
    use zbus::zvariant::{OwnedFd, OwnedValue, Value};
    use zbus::{MatchRule, Message};

    let file = std::fs::File::open(copy).map_err(|e| OpenFailure::Other(format!("{e}")))?;
    let no_app = |what: &str, err: &dyn std::fmt::Display| {
        log::warn!("the OpenURI portal: {what}: {err}");
        OpenFailure::NoApp
    };
    let bus = Blocking::from(connection.clone());
    let Some(name) = connection.unique_name() else {
        return Err(no_app("no unique name", &"the connection has none"));
    };
    let token = format!("hoplodex_{}", crate::db::random_hex(8).unwrap_or_default());
    let handle = format!(
        "/org/freedesktop/portal/desktop/request/{}/{token}",
        name.as_str().trim_start_matches(':').replace('.', "_")
    );

    // 1. Subscribe, so an answer sent before the call returns is not missed.
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface("org.freedesktop.portal.Request")
        .and_then(|rule| rule.member("Response"))
        .and_then(|rule| rule.path(handle.as_str()))
        .map(|rule| rule.build())
        .map_err(|err| no_app("the match rule", &err))?;
    let answers = MessageIterator::for_match_rule(rule, &bus, None)
        .map_err(|err| no_app("subscribing", &err))?;
    let (answered, answer) = mpsc::channel::<u32>();
    std::thread::spawn(move || {
        // Ends with the first answer, or when the connection does. The route
        // may have returned long before: the portal's chooser can wait on the
        // user, and the answer is then no one's to read.
        for message in answers.flatten() {
            let message: Message = message;
            if let Ok((code, _results)) =
                message.body().deserialize::<(u32, HashMap<String, OwnedValue>)>()
            {
                let _ = answered.send(code);
                return;
            }
        }
    });

    // 2. The call.
    let portal = Proxy::new(
        &bus,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.OpenURI",
    )
    .map_err(|err| no_app("no proxy", &err))?;
    let options: HashMap<&str, Value<'_>> = HashMap::from([
        ("handle_token", Value::from(token.as_str())),
        ("writable", Value::from(false)),
    ]);
    let fd = OwnedFd::from(std::os::fd::OwnedFd::from(file));
    portal.call_method("OpenFile", &("", fd, options)).map_err(|err| no_app("OpenFile", &err))?;

    // 3. The answer.
    match answer.recv_timeout(timeout) {
        Ok(2) => Err(OpenFailure::NoApp),
        Ok(_) | Err(_) => Ok(()),
    }
}

#[cfg(all(target_os = "linux", app_opener))]
impl<R: tauri::Runtime> AppOpener<R> {
    fn open_through_the_portal(
        &self,
        path: &Path,
        timeout: std::time::Duration,
    ) -> Result<(), OpenFailure> {
        match self.portal() {
            Some(connection) => portal_open(&connection, path, timeout),
            None => Err(OpenFailure::NoApp),
        }
    }
}

/// `xdg-open` (Linux) and `open` (macOS) say whether the OS knows a program
/// for the file by their exit status, within moments of being started; a
/// program that holds the launcher for longer is running, which is a yes.
/// The launcher gets its own process group, so nothing HoploDex does or
/// suffers reaches it, and a thread reaps it.
#[cfg(all(unix, app_opener))]
impl<R: tauri::Runtime> Opener for AppOpener<R> {
    fn open(&self, path: &Path) -> Result<(), OpenFailure> {
        #[cfg(target_os = "linux")]
        if let OpenerRoute::Portal { timeout } = self.route {
            return self.open_through_the_portal(path, timeout);
        }
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        use std::sync::mpsc;
        use std::time::Duration;

        #[cfg(target_os = "macos")]
        const LAUNCHER: &str = "open";
        #[cfg(not(target_os = "macos"))]
        const LAUNCHER: &str = "xdg-open";
        /// `xdg-open`: 3 "a required tool could not be found", 4 "the action
        /// failed", which is what it says when nothing is set for the type.
        #[cfg(not(target_os = "macos"))]
        fn no_app(code: i32) -> bool {
            matches!(code, 3 | 4)
        }
        /// `open` exits non-zero when no application can open the file.
        #[cfg(target_os = "macos")]
        fn no_app(_code: i32) -> bool {
            true
        }

        let mut child = Command::new(LAUNCHER)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|e| OpenFailure::Other(format!("{LAUNCHER}: {e}")))?;
        let (finished, status) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = finished.send(child.wait());
        });
        match status.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(Ok(status)) => match status.code() {
                Some(code) if no_app(code) => Err(OpenFailure::NoApp),
                _ => Err(OpenFailure::Other(format!("{LAUNCHER} failed ({status})"))),
            },
            Ok(Err(e)) => Err(OpenFailure::Other(format!("{LAUNCHER}: {e}"))),
            // Still running: the program it started is the user's now.
            Err(_) => Ok(()),
        }
    }
}

/// Whether Windows has a program registered to `open` files with this
/// extension (no dot), by `AssocQueryStringW` with `ASSOCF_INIT_IGNOREUNKNOWN`:
/// `HRESULT_FROM_WIN32(ERROR_NO_ASSOCIATION)` (0x80070483) means none. Any
/// other failure is not taken as "no app", so `ShellExecuteExW` decides.
#[cfg(windows)]
pub fn windows_has_app(extension: &str) -> bool {
    use windows_sys::Win32::UI::Shell::{
        ASSOCF_INIT_IGNOREUNKNOWN, ASSOCSTR_COMMAND, AssocQueryStringW,
    };

    /// `HRESULT_FROM_WIN32(ERROR_NO_ASSOCIATION)`.
    const NO_ASSOCIATION: i32 = 0x8007_0483_u32 as i32;

    let key: Vec<u16> = format!(".{extension}").encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let mut size: u32 = 0;
    // SAFETY: both strings are NUL-terminated and outlive the call; no output
    // buffer is passed, so `size` only receives the length needed.
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_INIT_IGNOREUNKNOWN,
            ASSOCSTR_COMMAND,
            key.as_ptr(),
            verb.as_ptr(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    result != NO_ASSOCIATION
}

/// Windows: `ShellExecuteExW` through `tauri-plugin-opener`. `has_app` is
/// asked first (007 T091 rework, 2026-10-07): for a type with no program (`.docx`
/// without Office), `ShellExecuteExW` succeeds and shows "How do you want to
/// open this file?", with or without `SEE_MASK_FLAG_NO_UI`, so it can't say so
/// itself. Its error for a type with no program, `SE_ERR_NOASSOC` (31) or
/// `ERROR_NO_ASSOCIATION` (1155), is still mapped, as a fallback.
#[cfg(all(windows, app_opener))]
impl<R: tauri::Runtime> Opener for AppOpener<R> {
    fn has_app(&self, extension: &str) -> bool {
        windows_has_app(extension)
    }

    fn open(&self, path: &Path) -> Result<(), OpenFailure> {
        use tauri_plugin_opener::OpenerExt;

        self.app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| match &e {
            tauri_plugin_opener::Error::Io(io) if matches!(io.raw_os_error(), Some(31 | 1155)) => {
                OpenFailure::NoApp
            }
            _ => OpenFailure::Other(e.to_string()),
        })
    }
}

/// An E2E build never hands the copy on, which would start a real viewer on
/// the test machine, and no harness can stub the default app everywhere:
/// Windows and macOS look it up in their own registrations, not on `PATH`
/// (#27). It appends the copy's path to `HOPLODEX_E2E_OPENED_LOG` instead,
/// a file in the sandbox, for the specs to check.
#[cfg(feature = "e2e")]
pub struct E2eOpener;

#[cfg(feature = "e2e")]
impl Opener for E2eOpener {
    fn open(&self, path: &Path) -> Result<(), OpenFailure> {
        use std::io::Write;

        let Some(log) = std::env::var_os("HOPLODEX_E2E_OPENED_LOG") else {
            return Ok(());
        };
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .and_then(|mut file| writeln!(file, "{}", path.display()))
            .map_err(|e| OpenFailure::Other(e.to_string()))
    }
}

// ---------------------------------------------------------------- the copy

/// A failure to write the copy: `what` of it, at `path`. A full disk is
/// `DISK_FULL`, naming `path`.
fn copy_error(what: &str, path: &Path, e: std::io::Error) -> CommandError {
    CommandError::from_io(&e, || {
        CommandError::new("INTERNAL_ERROR", format!("Could not prepare the document: {what}: {e}"))
    })
    .located_at(path)
}

/// Writes (or reuses) the copy of `document` that is handed to the other
/// app: `<root>/<id>/<stem>.<canonical extension>`, private from creation
/// (007 research.md §18, settles #71) and marked as from elsewhere. A copy that
/// is already there with the same length is reused, since documents never
/// change after attaching and Windows can't rewrite a file another program
/// holds open; any other is replaced.
fn write_copy(
    root: &Path,
    id: i64,
    document: &DocumentAttachment,
    document_type: &DocumentType,
) -> Result<PathBuf, CommandError> {
    let folder = root.join(id.to_string());
    if let Some(parent) = root.parent() {
        std::fs::create_dir_all(parent).map_err(|e| copy_error("its folder", parent, e))?;
    }
    private_dir(root)?;
    private_dir(&folder)?;
    protect_folder(&folder).map_err(|e| copy_error("its folder's protection", &folder, e))?;
    let path = folder.join(format!(
        "{}.{}",
        safe_stem(&document.original_filename),
        document_type.canonical_extension
    ));
    let bytes = &document.file_bytes;
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() && meta.len() == bytes.len() as u64 => return Ok(path),
        Ok(meta) if meta.is_dir() => {
            return Err(copy_error(
                "its file",
                &path,
                std::io::Error::other("a folder is in the way"),
            ));
        }
        // A link, or a file of another length: replaced.
        Ok(_) => std::fs::remove_file(&path).map_err(|e| copy_error("its file", &path, e))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(copy_error("its file", &path, e)),
    }
    create_private_file(&path, bytes).map_err(|e| copy_error("its file", &path, e))?;
    if let Err(e) = mark_untrusted(&path) {
        // A copy that can't be marked is not handed on.
        let _ = secure_delete_file(&path);
        return Err(copy_error("its mark", &path, e));
    }
    Ok(path)
}

/// Creates `path` (its parent exists) readable by the user alone, or checks
/// that what is there is such a folder. Unix: created `0700` with no window
/// in which it is wider, and an existing one is used only if it is a
/// directory, not a symbolic link, owned by the user.
fn private_dir(path: &Path) -> Result<(), CommandError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => private_dir_existing(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            #[cfg_attr(not(unix), allow(unused_mut))]
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(path) {
                Ok(()) => Ok(()),
                // Made by someone between the check and here: judged like
                // any folder that was already there.
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    private_dir_existing(path)
                }
                Err(e) => Err(copy_error("its folder", path, e)),
            }
        }
        Err(e) => Err(copy_error("its folder", path, e)),
    }
}

/// A folder that is already there is used only if it is a directory (not a
/// link) owned by the user.
fn private_dir_existing(path: &Path) -> Result<(), CommandError> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| copy_error("its folder", path, e))?;
    if !meta.is_dir() {
        return Err(copy_error("its folder", path, std::io::Error::other("it is not a folder")));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: `geteuid` has no preconditions and cannot fail.
        if meta.uid() != unsafe { libc::geteuid() } {
            return Err(copy_error(
                "its folder",
                path,
                std::io::Error::other("it belongs to another user"),
            ));
        }
    }
    Ok(())
}

/// Writes `bytes` to a new file at `path`, `0600` from creation on Unix.
fn create_private_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Not followed if something put a link there since the check, and
        // no wider than `0600` whatever the umask.
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    // A copy that couldn't be written whole (the disk filled) is not left
    // behind: it was made here, by `create_new`. Overwritten like every
    // other decrypted copy, which needs no new space.
    let written = file.write_all(bytes).and_then(|()| file.flush());
    if written.is_err() {
        drop(file);
        let _ = secure_delete_file(path);
    }
    written
}

/// The copy's file name without its extension: the stored name's last path
/// component with characters that are invalid on any supported OS replaced,
/// its own extension dropped (the canonical one is used, 007 FR-017), nothing
/// hidden, nothing a Windows name can't be.
fn safe_stem(original: &str) -> String {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let last_component = original.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = last_component
        .chars()
        .map(|c| if c.is_control() || r#"<>:"|?*"#.contains(c) { '_' } else { c })
        .collect();
    let stem = match cleaned.rsplit_once('.') {
        Some((stem, _)) if !stem.trim_matches('.').is_empty() => stem,
        _ => &cleaned,
    };
    let stem = stem.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let stem: String = stem.chars().take(100).collect();
    let stem = stem.trim_end_matches(|c: char| c == '.' || c.is_whitespace()).to_owned();
    if stem.is_empty() {
        "document".into()
    } else if RESERVED.contains(&stem.to_ascii_uppercase().as_str()) {
        format!("_{stem}")
    } else {
        stem
    }
}

// Per-OS marks on a copy and its folder. Each OS has its own function, so
// the sessions that finish them each touch one place.

/// Windows (007 T091): gives `folder` a protected DACL granting only the current
/// user (`SetSecurityInfo`), so nothing inherited from a relocated cache
/// folder widens it. Unix folders are `0700` from creation (`private_dir`);
/// macOS has the same.
#[cfg(not(windows))]
fn protect_folder(_folder: &Path) -> std::io::Result<()> {
    Ok(())
}

/// The folder's handle is opened without following a reparse point, and must
/// be a directory; its DACL becomes one ACE, full control for the current
/// user, inherited by the copy written into it, with inheritance from above
/// cut off.
#[cfg(windows)]
fn protect_folder(folder: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, SET_ACCESS, SetEntriesInAclW,
        SetSecurityInfo, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        SUB_CONTAINERS_AND_OBJECTS_INHERIT,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ALL_ACCESS, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, READ_CONTROL,
        WRITE_DAC,
    };

    let handle = std::fs::OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(folder)?;
    // Not a link or junction swapped in since `private_dir` looked.
    if !handle.metadata()?.is_dir() {
        return Err(std::io::Error::other("it is not a folder"));
    }
    let mut user = current_user_sid()?;
    let access = EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_ALL_ACCESS,
        grfAccessMode: SET_ACCESS,
        grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: user.as_mut_ptr().cast(),
        },
    };
    let mut acl: *mut ACL = std::ptr::null_mut();
    // SAFETY: one valid entry whose SID outlives the call; `acl` receives a
    // LocalAlloc'd ACL, freed below.
    let made = unsafe { SetEntriesInAclW(1, &access, std::ptr::null(), &mut acl) };
    if made != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(made as i32));
    }
    // SAFETY: the handle is open with WRITE_DAC and `acl` is the valid ACL
    // made above; owner, group and SACL are left as they are.
    let set = unsafe {
        SetSecurityInfo(
            handle.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            acl,
            std::ptr::null(),
        )
    };
    // SAFETY: `acl` came from SetEntriesInAclW and is freed once.
    unsafe { LocalFree(acl.cast()) };
    if set != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(set as i32));
    }
    Ok(())
}

/// The SID of the user the process runs as, copied out of its token.
#[cfg(windows)]
fn current_user_sid() -> std::io::Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        CopySid, GetLengthSid, GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: the pseudo-handle of this process; `token` receives a handle
    // closed below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let result = (|| {
        let mut needed = 0u32;
        // SAFETY: a size query with no buffer; it fails, setting `needed`.
        unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed) };
        // u64s, so the buffer is aligned for TOKEN_USER.
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        // SAFETY: `buffer` holds `needed` bytes.
        if unsafe {
            GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), needed, &mut needed)
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: filled with a TOKEN_USER, whose SID points into `buffer`.
        let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        // SAFETY: a valid SID, copied into a buffer of its length.
        let length = unsafe { GetLengthSid(sid) };
        let mut copy = vec![0u8; length as usize];
        if unsafe { CopySid(length, copy.as_mut_ptr().cast(), sid) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(copy)
    })();
    // SAFETY: opened above, closed once.
    unsafe { CloseHandle(token) };
    result
}

/// Marks the copy as having come from elsewhere, for the other app and the
/// OS to treat as untrusted (007 research.md §18). Linux has no such mark.
#[cfg(not(any(windows, target_os = "macos")))]
fn mark_untrusted(_copy: &Path) -> std::io::Result<()> {
    Ok(())
}

/// macOS (007 T090): adds the `com.apple.quarantine` attribute with `setxattr`
/// through `libc`, in the form a browser gives a download
/// (`flags;hex seconds;agent;event`, the flags Chrome's `0081`, no event in
/// the quarantine database), so the OS and the other app treat the copy as
/// from elsewhere. Not on a link: the copy was just created as a file.
#[cfg(target_os = "macos")]
fn mark_untrusted(copy: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let value = format!("0081;{seconds:08x};HoploDex;");
    let path = CString::new(copy.as_os_str().as_bytes())?;
    // SAFETY: both strings are NUL-terminated and outlive the call, and
    // `value` is valid for its length.
    let result = unsafe {
        libc::setxattr(
            path.as_ptr(),
            c"com.apple.quarantine".as_ptr(),
            value.as_ptr().cast(),
            value.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if result == 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
}

/// Windows: a `:Zone.Identifier` stream holding `ZoneId=3` (the Internet
/// zone), what a browser writes on a download, so Office opens the copy in
/// Protected View and SmartScreen applies.
#[cfg(windows)]
fn mark_untrusted(copy: &Path) -> std::io::Result<()> {
    let mut stream = copy.as_os_str().to_owned();
    stream.push(":Zone.Identifier");
    std::fs::write(stream, "[ZoneTransfer]\r\nZoneId=3\r\n")
}

#[tauri::command]
pub async fn list_documents(
    owner: RecordRef,
    session: ScopedSession,
    pdf: State<'_, PdfAvailabilityState>,
) -> Result<Vec<DocumentSummary>, CommandError> {
    let pdf = pdf.get();
    session.read(|conn| {
        Ok(ops::list_documents(conn, owner)?
            .into_iter()
            .map(|document| DocumentSummary::new(document, &pdf))
            .collect())
    })
}

/// The document types, for the picker and the drop router.
#[tauri::command]
pub async fn list_document_types() -> Result<&'static [DocumentType], CommandError> {
    Ok(ops::list_document_types())
}

#[tauri::command]
pub async fn add_document(
    owner: RecordRef,
    file_bytes: Vec<u8>,
    original_filename: String,
    session: ScopedSession,
    pdf: State<'_, PdfAvailabilityState>,
) -> Result<DocumentSummary, CommandError> {
    let pdf = pdf.get();
    session.write(|conn| {
        ops::add_document(conn, owner, &file_bytes, &original_filename)
            .map(|document| DocumentSummary::new(document, &pdf))
    })
}

/// Attaches a document from a file on disk: what a drop onto the window
/// delivers.
#[tauri::command]
pub async fn add_document_from_path(
    owner: RecordRef,
    path: String,
    session: ScopedSession,
    pdf: State<'_, PdfAvailabilityState>,
    reach: State<'_, Arc<Reach>>,
) -> Result<DocumentSummary, CommandError> {
    let pdf = pdf.get();
    session.write(|conn| {
        ops::add_document_from_path(conn, owner, Path::new(&path), &reach)
            .map(|document| DocumentSummary::new(document, &pdf))
    })
}

#[tauri::command]
pub async fn delete_document(
    id: i64,
    confirmed: bool,
    session: ScopedSession,
) -> Result<DeleteResult, CommandError> {
    // The preview of the document goes first: its bytes, helper and surface
    // must not outlive the document (007 contracts/tauri-commands.md
    // `delete_document`). Nothing is closed for a refused deletion.
    if confirmed {
        crate::commands::preview::ops::close_preview_of(&session.session, session.id, id)?;
    }
    session.write(|conn| ops::delete_document(conn, id, confirmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_name_becomes_a_safe_stem() {
        assert_eq!(safe_stem("receipt.pdf"), "receipt");
        assert_eq!(safe_stem("receipt.Pdf"), "receipt");
        assert_eq!(safe_stem("../../escape:me?.pdf"), "escape_me_");
        assert_eq!(safe_stem("a\\b/c d.tar.gz"), "c d.tar");
        assert_eq!(safe_stem("no extension"), "no extension");
        // Nothing hidden, empty, or a name Windows can't have.
        assert_eq!(safe_stem(".pdf"), "pdf");
        assert_eq!(safe_stem(".."), "document");
        assert_eq!(safe_stem(""), "document");
        assert_eq!(safe_stem("trailing. .pdf"), "trailing");
        assert_eq!(safe_stem("CON.pdf"), "_CON");
        assert_eq!(safe_stem(&format!("{}.pdf", "x".repeat(300))).chars().count(), 100);
    }

    // 011 FR-008, 011 US1-7: the route is chosen once from the reach.
    #[cfg(app_opener)]
    #[test]
    fn a_flatpak_hands_a_document_to_the_portal_and_an_unconfined_run_to_the_launcher() {
        use super::{OpenerRoute, opener_route};
        use crate::services::sandbox::UserDirs;

        let home = Path::new("/home/someone");
        let flatpak =
            Reach::from_flatpak_info("[Context]\nfilesystems=home;\n", home, &UserDirs::default());
        assert!(flatpak.is_flatpak());
        assert_eq!(opener_route(&Reach::Unconfined), OpenerRoute::XdgOpen);
        #[cfg(target_os = "linux")]
        assert_eq!(
            opener_route(&flatpak),
            OpenerRoute::Portal { timeout: std::time::Duration::from_secs(5) }
        );
        // Only Linux has a Flatpak, and nowhere else is there a portal route.
        #[cfg(not(target_os = "linux"))]
        assert_eq!(opener_route(&flatpak), OpenerRoute::XdgOpen);
    }
}
