//! WebView2's general autofill on Windows (code scanning alert
//! `plaintext-lifetime/cwe-312` on `main`).
//!
//! WebView2 saves what is typed into a form's fields when the form is sent and
//! leaves the page, and every HoploDex form does that: the values, serial
//! numbers included, went unencrypted into the main web view profile's
//! `Web Data` file and outlived the lock, the close and the record. The main
//! window turns general autofill off (`general_autofill_enabled(false)` in
//! `main.rs`), so nothing more is saved; [`clear_saved`] deletes what earlier
//! runs saved, at every start.

use tauri::{Runtime, WebviewWindow};

/// Asks the window's WebView2 profile to delete its general autofill data.
/// It runs on its own after this returns; a failure is logged, and leaves
/// only what earlier runs saved, since nothing more is saved.
pub fn clear_saved<R: Runtime>(window: &WebviewWindow<R>) {
    use ::windows::core::{Interface, Result as ComResult};
    use webview2_com::ClearBrowsingDataCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_BROWSING_DATA_KINDS_GENERAL_AUTOFILL, ICoreWebView2_13, ICoreWebView2Profile2,
    };

    let asked = window.with_webview(|platform| {
        // SAFETY: COM calls on the web view's own objects, made on the thread
        // that owns them.
        let asked = (|| unsafe {
            let profile = platform
                .controller()
                .CoreWebView2()?
                .cast::<ICoreWebView2_13>()?
                .Profile()?
                .cast::<ICoreWebView2Profile2>()?;
            profile.ClearBrowsingData(
                COREWEBVIEW2_BROWSING_DATA_KINDS_GENERAL_AUTOFILL,
                &ClearBrowsingDataCompletedHandler::create(Box::new(|done: ComResult<()>| {
                    match done {
                        Ok(()) => log::debug!("the saved autofill data was deleted"),
                        Err(err) => log::error!("the saved autofill data wasn't deleted: {err}"),
                    }
                    Ok(())
                })),
            )
        })();
        if let Err(err) = asked {
            log::error!("the saved autofill data couldn't be deleted: {err}");
        }
    });
    if let Err(err) = asked {
        log::error!("the saved autofill data couldn't be deleted: {err}");
    }
}
