//! Whether PDF preview is available, and why not (research.md §7, §17).

use std::sync::{Arc, Mutex};

use crate::services::machine_settings::MachineSettings;

/// Why PDF preview is off on this computer (FR-003a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnavailableReason {
    /// A copy written by the PDF viewer was caught during this version of
    /// HoploDex; PDF preview stays off until a different version starts.
    Held,
    /// The startup check of the web view's PDF switches failed.
    CheckFailed,
    /// The web view has no PDF viewer: a surface turned its PDF into a
    /// download this run.
    NoViewer,
}

impl UnavailableReason {
    /// The reason's sentence, shown after "PDFs can't be previewed on this
    /// computer." (ui contract §3).
    pub fn message(self) -> &'static str {
        match self {
            Self::Held => {
                "This computer's PDF viewer saved a copy of a document to disk, so PDF previews \
                 are off until HoploDex is updated."
            }
            Self::CheckFailed => {
                "HoploDex couldn't confirm that this computer's PDF viewer is kept from saving \
                 copies of documents to disk."
            }
            Self::NoViewer => "This computer's web view has no built-in PDF viewer.",
        }
    }
}

/// Whether PDFs are previewed on this computer in this run (data-model.md
/// "In memory, per run"). TIFF and text are unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfAvailability {
    Available,
    Unavailable { reason: UnavailableReason },
}

impl PdfAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Decides at startup (research.md §7, §17): this version's hold keeps
    /// PDF preview off, then the OS check. A hold for another version is
    /// cleared, so this version tries again. `os_check_passed` is
    /// [`os_check`]'s answer.
    pub fn at_startup(
        machine: &MachineSettings,
        running_version: &str,
        os_check_passed: bool,
    ) -> Self {
        machine.clear_stale_hold(running_version);
        let held = machine
            .pdf_preview_hold()
            .is_some_and(|hold| hold.version.as_deref() == Some(running_version));
        if held {
            Self::Unavailable { reason: UnavailableReason::Held }
        } else if !os_check_passed {
            Self::Unavailable { reason: UnavailableReason::CheckFailed }
        } else {
            Self::Available
        }
    }
}

/// [`PdfAvailability`] as Tauri state: decided at startup and changed at run
/// time (`NoViewer`, and `Held` when the watch catches a copy).
///
/// A handle to shared state: a clone is the same availability, so the
/// preview commands' `PreviewEnv` and the document list read one value.
#[derive(Debug, Clone)]
pub struct PdfAvailabilityState(Arc<Mutex<PdfAvailability>>);

impl PdfAvailabilityState {
    pub fn new(initial: PdfAvailability) -> Self {
        Self(Arc::new(Mutex::new(initial)))
    }

    pub fn get(&self) -> PdfAvailability {
        *self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set(&self, availability: PdfAvailability) {
        *self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = availability;
    }
}

impl PdfAvailabilityState {
    /// The watch caught a copy the PDF viewer wrote (macOS, research.md §7):
    /// PDF preview stays off until a different version of HoploDex runs. The
    /// hold is written to `machine.json` and takes effect now.
    pub fn hold(&self, machine: &MachineSettings) {
        machine.hold_pdf_preview(env!("CARGO_PKG_VERSION"));
        self.set(PdfAvailability::Unavailable { reason: UnavailableReason::Held });
    }

    /// A surface's web view turned its PDF into a download: it has no
    /// built-in PDF viewer (research.md §4).
    pub fn mark_no_viewer(&self) {
        self.set(PdfAvailability::Unavailable { reason: UnavailableReason::NoViewer });
    }
}

impl Default for PdfAvailabilityState {
    fn default() -> Self {
        Self::new(PdfAvailability::Available)
    }
}

/// Decides at startup, in `setup()` before anything is shown: this version's
/// hold, then [`os_check`] (research.md §7).
pub fn decide_at_startup(machine: &MachineSettings) -> PdfAvailability {
    PdfAvailability::at_startup(machine, env!("CARGO_PKG_VERSION"), os_check())
}

/// Whether this computer's web view has what the PDF surface needs to be held
/// to FR-003 and FR-004 (research.md §7), logging why not.
///
/// An E2E build (`e2e` feature only) reads `HOPLODEX_E2E_PDF_PREVIEW=off` to
/// fail it, so the frontend's handling can be tested and photographed.
pub fn os_check() -> bool {
    #[cfg(feature = "e2e")]
    if std::env::var("HOPLODEX_E2E_PDF_PREVIEW").is_ok_and(|v| v == "off") {
        log::warn!("PDF preview is switched off for this E2E run");
        return false;
    }
    let passed = platform_check();
    if !passed {
        log::warn!("the PDF viewer check failed; PDF preview is off for this run");
    }
    passed
}

/// WebKitGTK ships PDF.js from 2.40 (research.md §3).
#[cfg(target_os = "linux")]
fn platform_check() -> bool {
    // SAFETY: both only read the version WebKitGTK was built as.
    let (major, minor) = unsafe {
        (webkit2gtk::ffi::webkit_get_major_version(), webkit2gtk::ffi::webkit_get_minor_version())
    };
    (major, minor) >= (2, 40)
}

/// `+[WKPreferences _features]` lists `PDFPluginHUDEnabled`, the SPI the
/// surface switches the HUD and WebRTC off with respond, and a `WKPreferences`
/// reads both back as off after they are set (research.md §7). Must run on
/// the main thread (`setup()` does).
///
/// Not run on the machine this was written on: macOS only.
#[cfg(target_os = "macos")]
fn platform_check() -> bool {
    use objc2::{ClassType, MainThreadMarker, msg_send, rc::Retained, runtime::AnyObject, sel};
    use objc2_foundation::{NSArray, NSString};
    use objc2_web_kit::WKPreferences;

    let Some(mtm) = MainThreadMarker::new() else {
        log::error!("the PDF viewer check must run on the main thread");
        return false;
    };
    let prefs = unsafe { WKPreferences::new(mtm) };
    let class_lists_features: bool =
        unsafe { msg_send![WKPreferences::class(), respondsToSelector: sel!(_features)] };
    let instance_responds =
        |selector| -> bool { unsafe { msg_send![&*prefs, respondsToSelector: selector] } };
    if !class_lists_features
        || !instance_responds(sel!(_setEnabled:forFeature:))
        || !instance_responds(sel!(_isEnabledForFeature:))
        || !instance_responds(sel!(_setPeerConnectionEnabled:))
        || !instance_responds(sel!(_peerConnectionEnabled))
    {
        return false;
    }
    let features: Retained<NSArray<AnyObject>> =
        unsafe { msg_send![WKPreferences::class(), _features] };
    let hud = features.iter().find(|feature| {
        let key: Retained<NSString> = unsafe { msg_send![&**feature, key] };
        key.to_string() == "PDFPluginHUDEnabled"
    });
    let Some(hud) = hud else { return false };
    unsafe {
        let _: () = msg_send![&*prefs, _setEnabled: false, forFeature: &*hud];
        let _: () = msg_send![&*prefs, _setPeerConnectionEnabled: false];
        let hud_on: bool = msg_send![&*prefs, _isEnabledForFeature: &*hud];
        let rtc_on: bool = msg_send![&*prefs, _peerConnectionEnabled];
        !hud_on && !rtc_on
    }
}

/// The first WebView2 runtime that has every interface the surface uses.
/// Microsoft's API reference dates each to the SDK release that introduced
/// it, and an SDK's APIs need the runtime of the same build or later:
/// `ICoreWebView2Settings3` (`AreBrowserAcceleratorKeysEnabled`) 1.0.864.35,
/// `ICoreWebView2Settings7` (`HiddenPdfToolbarItems`) and `ICoreWebView2_11`
/// (`ContextMenuRequested`) 1.0.1185.39, and `ICoreWebView2_25`
/// (`SaveAsUIShowing`, the newest) 1.0.2739.15, which is runtime 128.0.2739.15.
/// The surface also queries each interface when it is built, so a threshold
/// set too low fails there instead (research.md §7).
#[cfg(windows)]
const MIN_WEBVIEW2_VERSION: windows::core::PCWSTR = windows::core::w!("128.0.2739.15");

/// The installed WebView2 runtime is at least [`MIN_WEBVIEW2_VERSION`].
///
/// Run on Windows Server 2025 with runtime 154.0.4258.62.
#[cfg(windows)]
fn platform_check() -> bool {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        CompareBrowserVersions, GetAvailableCoreWebView2BrowserVersionString,
    };
    use windows::core::{PCWSTR, PWSTR};

    let mut installed = PWSTR::null();
    // SAFETY: a null folder asks for the installed runtime; `installed`
    // receives a COM-allocated string, which `take_pwstr` frees.
    if unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut installed) }
        .is_err()
        || installed.is_null()
    {
        return false;
    }
    let mut order = 0i32;
    // SAFETY: `installed` is a valid, null-terminated string until freed below.
    let compared =
        unsafe { CompareBrowserVersions(PCWSTR(installed.0), MIN_WEBVIEW2_VERSION, &mut order) };
    drop(webview2_com::take_pwstr(installed));
    compared.is_ok() && order >= 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_and_a_no_viewer_turn_pdf_preview_off_for_the_run() {
        let config = tempfile::TempDir::new().unwrap();
        let machine = MachineSettings::load(config.path()).unwrap();
        let state = PdfAvailabilityState::default();

        state.mark_no_viewer();
        assert_eq!(
            state.get(),
            PdfAvailability::Unavailable { reason: UnavailableReason::NoViewer }
        );

        state.hold(&machine);
        assert_eq!(state.get(), PdfAvailability::Unavailable { reason: UnavailableReason::Held });
        assert_eq!(
            machine.pdf_preview_hold().unwrap().version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(
            decide_at_startup(&machine),
            PdfAvailability::Unavailable { reason: UnavailableReason::Held }
        );
    }

    /// The test machine has a WebView2 runtime far newer than the minimum.
    #[cfg(windows)]
    #[test]
    fn the_windows_check_reads_the_installed_runtimes_version() {
        assert!(platform_check());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_linux_check_reads_webkitgtks_version() {
        // Built against WebKitGTK 2.40 or later (Cargo.toml's `v2_40`).
        assert!(platform_check());
    }
}
