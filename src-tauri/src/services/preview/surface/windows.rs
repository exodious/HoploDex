//! The surface on Windows: WebView2's settings, browser arguments and
//! events (research.md §6-§10).
//!
//! `customize` gives the surface its own browser process and its proxy.
//! `attach` then reaches the web view's COM interfaces through Tauri's
//! `with_webview` (`webview2-com`, the crate wry uses), on the main thread,
//! and sets what research.md §6-§10 ask of Edge: the request filter, the PDF
//! toolbar without Save, Save As and Print, Save As cancelled, a context menu
//! of Copy only, the browser accelerator keys and developer tools off, and
//! `AcceleratorKeyPressed` for Escape, F6 and key activity. Each interface is
//! queried first, and one the installed runtime lacks refuses the surface
//! (the startup check in `availability.rs` should have kept it from getting
//! this far). A thread samples `GetLastInputInfo` for the idle lock while
//! HoploDex is the foreground window.

use tauri::{Manager, Runtime, Url, Webview, Window, webview::WebviewBuilder};

use std::{
    path::Path,
    sync::{Arc, Mutex, Weak, mpsc},
    thread,
    time::Duration,
};

use ::windows::core::{HSTRING, Interface, PWSTR, Result as ComResult};
use webview2_com::{
    AcceleratorKeyPressedEventHandler, ContextMenuRequestedEventHandler,
    Microsoft::Web::WebView2::Win32::*, SaveAsUIShowingEventHandler,
    WebResourceRequestedEventHandler, take_pwstr,
};
use windows_sys::Win32::{
    System::{SystemInformation::GetTickCount, Threading::GetCurrentProcessId},
    UI::{
        Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    },
};

use super::{Hooks, Rect};

/// How long `attach` waits for the main thread to set the web view up.
const ATTACH_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the foreground window's input is sampled (research.md §10).
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

/// Input this recent, while HoploDex is the foreground window, is activity.
/// A little over the sample interval, so no sample misses one.
const RECENT_INPUT_MS: u32 = 1500;

/// `VK_ESCAPE` and `VK_F6`.
const VK_ESCAPE: u32 = 0x1B;
const VK_F6: u32 = 0x75;

/// wry's default browser arguments, which it adds only when an app gives
/// none (wry 0.55's `webview2/mod.rs`), so they are written out here.
const WRY_DEFAULT_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// The surface's browser arguments, in full (research.md §6): wry's defaults,
/// the proxy (which also takes loopback, and is the layer that covers Edge's
/// viewer frames, which the filter can't see) and WebRTC kept to the proxy.
/// `proxy_url` itself is not used, since wry would add it only to arguments
/// nobody gave.
fn browser_args(proxy_url: &Url) -> String {
    let proxy = format!(
        "{}:{}",
        proxy_url.host_str().unwrap_or("127.0.0.1"),
        proxy_url.port_or_known_default().unwrap_or(80)
    );
    format!(
        "{WRY_DEFAULT_ARGS} --proxy-server=http://{proxy} --proxy-bypass-list=<-loopback> \
         --webrtc-ip-handling-policy=disable_non_proxied_udp"
    )
}

/// What the surface may request: its own document (`document`, the one URL
/// the surface may show, which `surface/mod.rs` keeps current: the same
/// address, whatever fragment) and the URLs that name nothing outside the page
/// (`blob:`, `data:`, `about:`). Everything else is answered with a 403
/// (research.md §6). The filter sees only the top frame, whose one request
/// besides the document is for the viewer it embeds: Edge's viewer and its
/// frames are not seen by it (the proxy is the layer there).
fn request_allowed(uri: &str, document: &Url) -> bool {
    if ["blob:", "data:", "about:"].iter().any(|scheme| uri.starts_with(scheme)) {
        return true;
    }
    let Ok(mut requested) = Url::parse(uri) else { return false };
    let mut document = document.clone();
    requested.set_fragment(None);
    document.set_fragment(None);
    requested == document
}

/// A user data folder of its own, so the surface has a browser process of its
/// own: browser arguments belong to that process, which every web view
/// sharing a folder shares (research.md §6).
pub(super) fn customize<R: Runtime>(
    builder: WebviewBuilder<R>,
    proxy_url: &Url,
    data_directory: &Path,
) -> WebviewBuilder<R> {
    builder
        .data_directory(data_directory.to_path_buf())
        .additional_browser_args(&browser_args(proxy_url))
}

/// A string out-parameter's value, or "" if the call fails.
fn text(f: impl FnOnce(&mut PWSTR) -> ComResult<()>) -> String {
    let mut p = PWSTR::null();
    if f(&mut p).is_err() {
        return String::new();
    }
    take_pwstr(p)
}

fn missing(name: &'static str) -> impl FnOnce(::windows::core::Error) -> String {
    move |e| format!("this computer's WebView2 runtime has no {name} ({e})")
}

/// Sets the web view up, on the main thread: every interface is queried
/// before anything is set, so a runtime that lacks one changes nothing. The
/// handlers hold `hooks` for as long as the web view lives.
fn configure(
    platform: &tauri::webview::PlatformWebview,
    hooks: &Arc<Hooks>,
    document: &Arc<Mutex<Url>>,
) -> Result<(), String> {
    let err = |e: ::windows::core::Error| e.to_string();
    // SAFETY: COM calls on the web view's own objects, made on the thread
    // that owns them; each out-parameter is a local that outlives its call.
    unsafe {
        let environment = platform.environment();
        let controller = platform.controller();
        let webview = controller.CoreWebView2().map_err(err)?;
        let settings = webview.Settings().map_err(err)?;
        let settings3: ICoreWebView2Settings3 =
            settings.cast().map_err(missing("ICoreWebView2Settings3"))?;
        let settings7: ICoreWebView2Settings7 =
            settings.cast().map_err(missing("ICoreWebView2Settings7"))?;
        let webview11: ICoreWebView2_11 = webview.cast().map_err(missing("ICoreWebView2_11"))?;
        let webview25: ICoreWebView2_25 = webview.cast().map_err(missing("ICoreWebView2_25"))?;

        settings3.SetAreBrowserAcceleratorKeysEnabled(false).map_err(err)?;
        settings.SetAreDevToolsEnabled(false).map_err(err)?;
        settings.SetAreDefaultScriptDialogsEnabled(false).map_err(err)?;
        settings7
            .SetHiddenPdfToolbarItems(COREWEBVIEW2_PDF_TOOLBAR_ITEMS(
                COREWEBVIEW2_PDF_TOOLBAR_ITEMS_SAVE.0
                    | COREWEBVIEW2_PDF_TOOLBAR_ITEMS_SAVE_AS.0
                    | COREWEBVIEW2_PDF_TOOLBAR_ITEMS_PRINT.0,
            ))
            .map_err(err)?;

        // Requests: every one the web view makes is seen, and all but the
        // surface's own are refused. The newer filter also covers workers;
        // the older one is the top frame's documents (the proxy is the layer
        // under both, research.md §6).
        match webview.cast::<ICoreWebView2_22>() {
            Ok(webview22) => webview22
                .AddWebResourceRequestedFilterWithRequestSourceKinds(
                    &HSTRING::from("*"),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                    COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
                )
                .map_err(err)?,
            Err(_) => webview
                .AddWebResourceRequestedFilter(
                    &HSTRING::from("*"),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                )
                .map_err(err)?,
        }
        let document = Arc::clone(document);
        let mut token = 0i64;
        webview
            .add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let current = document.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    if request_allowed(&text(|p| args.Request()?.Uri(p)), &current) {
                        return Ok(());
                    }
                    let refusal = environment.CreateWebResourceResponse(
                        None,
                        403,
                        &HSTRING::from("Blocked"),
                        &HSTRING::new(),
                    )?;
                    args.SetResponse(&refusal)
                })),
                &mut token,
            )
            .map_err(err)?;

        // Save As is cancelled, whatever raised it.
        webview25
            .add_SaveAsUIShowing(
                &SaveAsUIShowingEventHandler::create(Box::new(|_, args| {
                    let Some(args) = args else { return Ok(()) };
                    args.SetSuppressDefaultDialog(true)?;
                    args.SetCancel(true)
                })),
                &mut token,
            )
            .map_err(err)?;

        // A context menu of Copy only.
        webview11
            .add_ContextMenuRequested(
                &ContextMenuRequestedEventHandler::create(Box::new(|_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let items = args.MenuItems()?;
                    let mut count = 0u32;
                    items.Count(&mut count)?;
                    for index in (0..count).rev() {
                        let name = items.GetValueAtIndex(index)?;
                        if text(|p| name.Name(p)) != "copy" {
                            items.RemoveValueAtIndex(index)?;
                        }
                    }
                    items.Count(&mut count)?;
                    if count == 0 {
                        // Nothing to show, so no menu.
                        args.SetHandled(true)?;
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(err)?;

        // Escape and F6 are the app's, and any key is activity.
        let keys = Arc::clone(hooks);
        controller
            .add_AcceleratorKeyPressed(
                &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                    let mut key = 0u32;
                    args.KeyEventKind(&mut kind)?;
                    args.VirtualKey(&mut key)?;
                    let down = kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                        || kind == COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN;
                    match key {
                        VK_ESCAPE | VK_F6 => {
                            // Consumed on release too, so the viewer sees neither.
                            args.SetHandled(true)?;
                            if down {
                                if key == VK_ESCAPE {
                                    keys.escape();
                                } else {
                                    keys.focus_chrome();
                                }
                            }
                        }
                        _ if down => keys.activity(),
                        _ => {}
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(err)?;
    }
    Ok(())
}

/// Whether the foreground window belongs to this process.
fn hoplodex_is_foreground() -> bool {
    // SAFETY: plain queries; the process id is written to a local.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return false;
        }
        let mut process = 0u32;
        GetWindowThreadProcessId(window, &mut process);
        process == GetCurrentProcessId()
    }
}

/// How long ago the last keyboard or mouse input was, system-wide, in ms.
fn input_age_ms() -> Option<u32> {
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: `info` is a local with its size set.
    (unsafe { GetLastInputInfo(&mut info) } != 0)
        .then(|| unsafe { GetTickCount() }.wrapping_sub(info.dwTime))
}

/// Samples the input clock each second and passes recent input on as
/// activity while HoploDex is the foreground window (research.md §10), until
/// the surface is gone: its web view is no longer in the window, or the
/// hooks have been let go of.
fn watch_input<R: Runtime>(webview: Webview<R>, hooks: Weak<Hooks>) {
    let spawned = thread::Builder::new().name("preview-input".into()).spawn(move || {
        loop {
            thread::sleep(SAMPLE_INTERVAL);
            let Some(hooks) = hooks.upgrade() else { return };
            if webview.window().get_webview(super::LABEL).is_none() {
                return;
            }
            if hoplodex_is_foreground() && input_age_ms().is_some_and(|age| age < RECENT_INPUT_MS) {
                hooks.activity();
            }
        }
    });
    if let Err(e) = spawned {
        log::warn!("could not start the preview surface's input sampler: {e}");
    }
}

/// Sets the web view up (see the module's comment), refusing the surface if
/// the runtime can't. `document` is the one URL the surface may show, which
/// `surface/mod.rs` keeps current and the request filter follows. Call it from a thread other than the main one: it
/// waits for the main thread.
pub(super) fn attach<R: Runtime>(
    _window: &Window<R>,
    webview: &Webview<R>,
    hooks: &Arc<Hooks>,
    document: &Arc<Mutex<Url>>,
) -> tauri::Result<()> {
    let (done, result) = mpsc::channel::<Result<(), String>>();
    let for_handlers = Arc::clone(hooks);
    let document = Arc::clone(document);
    webview.with_webview(move |platform| {
        let _ = done.send(configure(&platform, &for_handlers, &document));
    })?;
    match result.recv_timeout(ATTACH_TIMEOUT) {
        Ok(Ok(())) => {}
        Ok(Err(why)) => return Err(std::io::Error::other(why).into()),
        Err(_) => {
            return Err(std::io::Error::other("the web view was not set up in time").into());
        }
    }
    watch_input(webview.clone(), Arc::downgrade(hooks));
    Ok(())
}

pub(super) fn place<R: Runtime>(
    webview: &Webview<R>,
    bounds: Rect,
    visible: bool,
) -> tauri::Result<()> {
    super::place_with_tauri(webview, bounds, visible)
}

pub(super) fn bounds<R: Runtime>(webview: &Webview<R>) -> tauri::Result<Rect> {
    super::bounds_from_tauri(webview)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_args_hold_wrys_defaults_and_the_proxy() {
        let args = browser_args(&"http://127.0.0.1:4242".parse().unwrap());
        assert!(args.starts_with(WRY_DEFAULT_ARGS));
        assert!(args.contains("--proxy-server=http://127.0.0.1:4242 "));
        assert!(args.contains("--proxy-bypass-list=<-loopback>"));
        assert!(args.contains("--webrtc-ip-handling-policy=disable_non_proxied_udp"));
    }

    #[test]
    fn only_the_surfaces_current_document_and_page_local_urls_may_be_requested() {
        let document: Url = "http://hdpreview.localhost/abc/document.pdf".parse().unwrap();
        assert!(request_allowed("http://hdpreview.localhost/abc/document.pdf", &document));
        assert!(request_allowed("http://hdpreview.localhost/abc/document.pdf#page=2", &document));
        assert!(request_allowed("blob:http://hdpreview.localhost/1234", &document));
        assert!(request_allowed("about:blank", &document));
        // Another document of the origin, its other paths and everything else.
        assert!(!request_allowed("http://hdpreview.localhost/abc/other.pdf", &document));
        assert!(!request_allowed("http://hdpreview.localhost/__report?k=x", &document));
        assert!(!request_allowed("http://hdpreview.localhost/abc/document.pdf?x=1", &document));
        assert!(!request_allowed("https://example.com/", &document));
        assert!(!request_allowed(
            "http://hdpreview.localhost.evil.test/abc/document.pdf",
            &document
        ));
        assert!(!request_allowed("http://127.0.0.1:4242/", &document));
        assert!(!request_allowed("chrome-extension://abc/index.html", &document));
        assert!(!request_allowed("not a url", &document));
    }
}
