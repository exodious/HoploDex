//! The native confirmation's text and the session's consent rule
//! (research.md §16, §17; ui contract §4).
//!
//! Handing a document to another app is decided by the user in a native
//! dialog that Rust shows, with text Rust builds from the stored file name, so
//! a compromised web view can neither answer it nor put another name in front
//! of the user. [`Consent`] is the seam: the app's [`DialogConsent`] shows the
//! dialog, an E2E build's `E2eConsent` answers from the environment, and the
//! tests' fake records what it was asked.

use crate::models::document_opening::DocumentOpening;

/// How the user answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentAnswer {
    /// "Open in another app".
    Open,
    /// "Cancel", Escape, or the dialog could not be shown: nothing is
    /// written and nothing starts.
    Cancel,
}

/// What the user is asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsentRequest {
    /// Opening one document (`open_document`).
    Document {
        /// The stored file name.
        name: String,
        /// The document type's label, "PDF" or "Word".
        kind: String,
        /// The setting is "Open in another app" and this session has not
        /// confirmed yet: the dialog adds that it won't ask again.
        first_of_session_with_external: bool,
    },
    /// Choosing "Open in another app" as this computer's setting
    /// (`set_document_opening`).
    Setting,
}

/// The longest file name the dialog's title shows, in characters.
const NAME_LIMIT: usize = 120;

/// The four consequences every confirmation lists (FR-009).
const CONSEQUENCES: [&str; 4] = [
    "Other apps, and anyone who can use this computer account, can read the copy while it is there.",
    "The other app may keep its own copies or records of the document, such as recent-files lists, \
     autosaves, caches or cloud sync. HoploDex can't find or delete those.",
    "The other app may connect to the internet if the document asks it to, for example to load a \
     picture or follow a link, which can show that the document was opened.",
    "HoploDex deletes its copy when this database is closed or locked or HoploDex quits, so the \
     other app may lose the document then. Changes saved to the copy are not kept in your \
     collection.",
];

impl ConsentRequest {
    /// The dialog's title.
    pub fn title(&self) -> String {
        match self {
            Self::Document { name, .. } => {
                format!("Open \u{201C}{}\u{201D} in another app?", display_name(name))
            }
            Self::Setting => "Open documents in another app?".to_owned(),
        }
    }

    /// The dialog's text.
    pub fn body(&self) -> String {
        let bullets = CONSEQUENCES.map(|line| format!("\u{2022} {line}")).join("\n");
        match self {
            Self::Document { kind, first_of_session_with_external, .. } => {
                let mut body = format!(
                    "HoploDex will put an unprotected copy of this document on this computer and \
                     open it in the app this computer uses for {kind} documents.\n\n{bullets}"
                );
                if *first_of_session_with_external {
                    body.push_str(
                        "\n\nYou won't be asked again until this database is closed or locked.",
                    );
                }
                body
            }
            Self::Setting => format!(
                "Each document you open will be copied, unprotected, to this computer and opened \
                 in the app this computer uses for its type. This applies to every database on \
                 this computer.\n\n{bullets}\n\nYou'll be asked once each time a database is \
                 opened or unlocked."
            ),
        }
    }
}

/// A stored name as the title shows it: control characters (and the
/// characters that reorder text, which can make one name read as another)
/// replaced, and cut to [`NAME_LIMIT`] characters with an ellipsis.
fn display_name(name: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if c.is_control() || is_bidi_control(c) { '_' } else { c }).collect();
    if cleaned.chars().count() <= NAME_LIMIT {
        return cleaned;
    }
    let mut cut: String = cleaned.chars().take(NAME_LIMIT - 1).collect();
    cut.push('\u{2026}');
    cut
}

fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Whether an external open must ask (research.md §17): always, unless the
/// setting is "Open in another app" and this session has already been
/// answered "Open in another app". Every external open goes through this.
pub fn needs_consent(setting: DocumentOpening, confirmed_this_session: bool) -> bool {
    !(setting == DocumentOpening::External && confirmed_this_session)
}

/// Asks the user. Blocks until they answer, so a caller must not hold the
/// session's lock (research.md §16).
pub trait Consent: Send + Sync {
    fn ask(&self, request: ConsentRequest) -> ConsentAnswer;
}

/// The app's confirmation: a native warning dialog on `tauri-plugin-dialog`,
/// parented to the main window.
pub struct DialogConsent<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> DialogConsent<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: tauri::Runtime> Consent for DialogConsent<R> {
    fn ask(&self, request: ConsentRequest) -> ConsentAnswer {
        use tauri::Manager;
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

        let (answered, answer) = std::sync::mpsc::channel();
        let mut dialog = self
            .app
            .dialog()
            .message(request.body())
            .title(request.title())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Open in another app".to_owned(),
                "Cancel".to_owned(),
            ));
        if let Some(window) = self.app.get_window("main") {
            dialog = dialog.parent(&window);
        }
        dialog.show(move |open| {
            let _ = answered.send(open);
        });
        // A dialog that never reports back (the app is going away) is a
        // Cancel: nothing is written.
        match answer.recv() {
            Ok(true) => ConsentAnswer::Open,
            _ => ConsentAnswer::Cancel,
        }
    }
}

/// The E2E build's stand-in for the dialog, which no WebDriver can click:
/// `HOPLODEX_E2E_CONSENT=open` answers "Open in another app", anything else
/// (or nothing) "Cancel", and each request's title is appended to the file
/// named by `HOPLODEX_E2E_CONSENT_LOG`, for the specs to read. Never in a
/// release build (`scripts/check-no-webdriver.mjs`).
#[cfg(feature = "e2e")]
pub struct E2eConsent;

#[cfg(feature = "e2e")]
impl Consent for E2eConsent {
    fn ask(&self, request: ConsentRequest) -> ConsentAnswer {
        use std::io::Write;

        if let Some(log) = std::env::var_os("HOPLODEX_E2E_CONSENT_LOG") {
            let written = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
                .and_then(|mut file| writeln!(file, "{}", request.title()));
            if let Err(err) = written {
                log::warn!("could not write the consent log: {err}");
            }
        }
        match std::env::var("HOPLODEX_E2E_CONSENT").as_deref() {
            Ok("open") => ConsentAnswer::Open,
            _ => ConsentAnswer::Cancel,
        }
    }
}
