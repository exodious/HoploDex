//! The surface on Linux: WebKitGTK's settings, filter and signals
//! (research.md §6-§10).
//!
//! Tauri's `Window::add_child` can't place a child web view on Linux: it packs
//! it into the window's `GtkBox` beside the main web view and ignores its
//! bounds (`pdf_surface_check` showed the PDF filling the lower half of the
//! window, research.md §4). So `attach` moves the main web view into a
//! `GtkOverlay` and the surface over it, and `place` positions the surface
//! there with margins and a size request, as GTK does it. GTK's C API is
//! called directly: `gtk` is not a dependency of this crate, and `webkit2gtk`
//! doesn't re-export it.
//!
//! `attach` also does what research.md §6 and §10 ask of WebKitGTK: WebRTC
//! off, the content filter that blocks every URL the surface may not load
//! (compiled into a store under the cache folder, installed before the
//! document is asked for, and the surface refused if it can't be), the
//! `print` signal answered as handled, a context menu of Copy and Select All
//! only, and the keys and mouse events that the app needs to know about.
//! Every signal handler runs on the GTK thread and returns at once: the
//! hooks it calls start their own threads for anything that waits on the
//! session.

use std::{
    ffi::{CStr, CString, c_void},
    os::raw::{c_char, c_int, c_uint},
    path::Path,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

use tauri::{Manager, Runtime, Url, Webview, Window, webview::WebviewBuilder};
use webkit2gtk::{
    ContextMenuExt, ContextMenuItemExt, SettingsExt, WebViewExt,
    glib::{
        self,
        ffi::{GList, g_list_free},
        gobject_ffi::{self, GObject, g_object_ref, g_object_unref, g_type_check_instance_is_a},
        translate::ToGlibPtr,
    },
};

use super::{Hooks, Rect};

type Widget = *mut c_void;
const ALIGN_START: c_int = 1;

// libgtk-3 is already linked: wry's `gtk` crate links it.
unsafe extern "C" {
    fn gtk_widget_get_toplevel(widget: Widget) -> Widget;
    fn gtk_window_get_focus(window: Widget) -> Widget;
    fn gtk_widget_grab_focus(widget: Widget);
    fn gtk_widget_get_parent(widget: Widget) -> Widget;
    fn gtk_container_get_children(container: Widget) -> *mut GList;
    fn gtk_bin_get_child(bin: Widget) -> Widget;
    fn gtk_container_remove(container: Widget, widget: Widget);
    fn gtk_container_add(container: Widget, widget: Widget);
    fn gtk_box_pack_start(b: Widget, child: Widget, expand: c_int, fill: c_int, padding: c_uint);
    fn gtk_overlay_new() -> Widget;
    fn gtk_overlay_get_type() -> usize;
    fn gtk_overlay_add_overlay(overlay: Widget, widget: Widget);
    fn gtk_overlay_set_overlay_pass_through(overlay: Widget, widget: Widget, pass: c_int);
    fn gtk_widget_set_halign(widget: Widget, align: c_int);
    fn gtk_widget_set_valign(widget: Widget, align: c_int);
    fn gtk_widget_set_margin_start(widget: Widget, margin: c_int);
    fn gtk_widget_set_margin_top(widget: Widget, margin: c_int);
    fn gtk_widget_get_margin_start(widget: Widget) -> c_int;
    fn gtk_widget_get_margin_top(widget: Widget) -> c_int;
    fn gtk_widget_set_size_request(widget: Widget, width: c_int, height: c_int);
    fn gtk_widget_get_allocated_width(widget: Widget) -> c_int;
    fn gtk_widget_get_allocated_height(widget: Widget) -> c_int;
    fn gtk_widget_show_all(widget: Widget);
    fn gtk_widget_set_visible(widget: Widget, visible: c_int);
}

/// The proxy: `proxy_url` on WebKitGTK sends loopback through it too (wry's
/// empty bypass list), so the tripwire sees everything (research.md §6).
///
/// The `data_directory` is not for a folder here (an incognito web view's
/// context is ephemeral and keeps nothing). It is what tells
/// tauri-runtime-wry's web context store that this is not the main window's
/// context: the store registers a custom protocol once for each key, on the
/// first web view of that key, and an incognito web view builds a context of
/// its own that the registration never reaches. Without a key of its own the
/// surface can't load an `hdpreview:` URL ("The URL can't be shown"), found
/// by `pdf_surface_check` (research.md §4).
///
/// The key must also be new for each surface: the store keeps a key's entry
/// for the life of the app on Linux, and a second surface under the same key
/// would find the protocol "already registered" and never get it, so every
/// PDF after the first would fail to load (found by
/// `us13-document-preview.e2e.ts`). Each surface leaves one small entry in
/// the store, and nothing on disk.
pub(super) fn customize<R: Runtime>(
    builder: WebviewBuilder<R>,
    proxy_url: &Url,
    data_directory: &Path,
) -> WebviewBuilder<R> {
    static SURFACES: AtomicUsize = AtomicUsize::new(0);
    let key = data_directory.join(format!("surface-{}", SURFACES.fetch_add(1, Ordering::Relaxed)));
    builder.proxy_url(proxy_url.clone()).data_directory(key)
}

/// Runs `f` with the surface's `GtkWidget`, on the GTK thread.
fn with_widget<R: Runtime>(
    webview: &Webview<R>,
    f: impl FnOnce(Widget) + Send + 'static,
) -> tauri::Result<()> {
    webview.with_webview(move |platform| {
        let view = platform.inner();
        let raw: *mut webkit2gtk::ffi::WebKitWebView = view.to_glib_none().0;
        f(raw.cast());
    })
}

/// The folder under the cache folder where the compiled content filter is
/// kept. It holds rules only, no document content.
const FILTER_STORE: &str = "preview-filter-store";

/// The identifier the filter is stored under.
const FILTER_ID: &str = "preview-block-all";

/// How long the filter may take to compile and install before the surface
/// is refused. WebKit compiles it on its own thread, in a few milliseconds.
const FILTER_TIMEOUT: Duration = Duration::from_secs(10);

/// The content filter (research.md §6): every URL is blocked but the
/// surface's own scheme and the PDF viewer's. `blob:`, `data:` and `about:`
/// name nothing outside the page and are what the viewer itself makes
/// (`pdf_surface_check`), so they stay.
const BLOCK_RULES: &str = r#"[
  {"trigger":{"url-filter":".*"},"action":{"type":"block"}},
  {"trigger":{"url-filter":"^hdpreview:"},"action":{"type":"ignore-previous-rules"}},
  {"trigger":{"url-filter":"^webkit-pdfjs-viewer:"},"action":{"type":"ignore-previous-rules"}},
  {"trigger":{"url-filter":"^blob:"},"action":{"type":"ignore-previous-rules"}},
  {"trigger":{"url-filter":"^data:"},"action":{"type":"ignore-previous-rules"}},
  {"trigger":{"url-filter":"^about:"},"action":{"type":"ignore-previous-rules"}}
]"#;

/// `GDK_KEY_Escape` and `GDK_KEY_F6`.
const KEY_ESCAPE: c_uint = 0xff1b;
const KEY_F6: c_uint = 0xffc3;

/// The start of `GdkEventKey`, as far as the key handler reads it.
#[repr(C)]
struct KeyEvent {
    kind: c_int,
    window: *mut c_void,
    send_event: i8,
    time: u32,
    state: c_uint,
    keyval: c_uint,
}

/// Configures the surface (settings, signals, filter), moves it into the
/// overlay over the main web view, and waits for the content filter to be
/// installed. Called once after `add_child`, from a thread other than the
/// main one. The caller closes the surface if this fails: a surface without
/// its filter must never load a document.
///
/// A web view that has been moved shows nothing until its page next changes,
/// so the other web views of the window are made to repaint.
pub(super) fn attach<R: Runtime>(
    window: &Window<R>,
    webview: &Webview<R>,
    hooks: &Arc<Hooks>,
) -> tauri::Result<()> {
    let store = crate::app_dirs::cache_dir(webview.app_handle())?.join(FILTER_STORE);
    std::fs::create_dir_all(&store)?;
    let store = CString::new(store.to_string_lossy().as_bytes())
        .map_err(|_| std::io::Error::other("the cache folder's path has a NUL in it"))?;
    let (installed, filter_result) = mpsc::channel::<Result<(), String>>();
    let hooks = Arc::clone(hooks);
    webview.with_webview(move |platform| {
        let view = platform.inner();
        let raw: *mut webkit2gtk::ffi::WebKitWebView = view.to_glib_none().0;
        harden(&view);
        connect_signals(&view, raw, &hooks);
        // SAFETY: `raw` is the live web view, and this runs on the GTK thread.
        unsafe {
            overlay(raw.cast());
            install_filter(raw, &store, installed);
        }
    })?;
    match filter_result.recv_timeout(FILTER_TIMEOUT) {
        Ok(Ok(())) => {}
        Ok(Err(why)) => return Err(std::io::Error::other(why).into()),
        Err(_) => {
            return Err(
                std::io::Error::other("the content filter was not installed in time").into()
            );
        }
    }
    for other in window.webviews().iter().filter(|w| w.label() != super::LABEL) {
        other.eval(REPAINT)?;
    }
    Ok(())
}

/// WebRTC off (research.md §6): off by default and absent in most builds,
/// but set explicitly.
fn harden(view: &webkit2gtk::WebView) {
    let Some(settings) = WebViewExt::settings(view) else {
        log::warn!("the preview surface has no settings to harden");
        return;
    };
    settings.set_enable_webrtc(false);
    if settings.enables_webrtc() {
        log::warn!("WebRTC could not be turned off in the preview surface");
    }
}

/// Printing refused, the context menu cut down to Copy and Select All, and
/// the input the app watches for (research.md §10).
fn connect_signals(
    view: &webkit2gtk::WebView,
    raw: *mut webkit2gtk::ffi::WebKitWebView,
    hooks: &Arc<Hooks>,
) {
    // Ctrl+P and the viewer's Print button both end here: answered as
    // handled, so no dialog or print job is made.
    view.connect_print(|_, _| true);
    view.connect_context_menu(|_, menu, _, _| {
        for item in menu.items() {
            let keep = !item.is_separator()
                && matches!(
                    item.stock_action(),
                    webkit2gtk::ContextMenuAction::Copy | webkit2gtk::ContextMenuAction::SelectAll
                );
            if !keep {
                menu.remove(&item);
            }
        }
        // Nothing left to offer: no menu at all.
        menu.n_items() == 0
    });
    let instance: *mut c_void = raw.cast();
    // SAFETY: each handler takes the widget, a GDK event and the `Arc<Hooks>`
    // given here, as the signal's C signature has them; the `Arc` is dropped
    // when the signal is disconnected with the widget.
    unsafe {
        connect(instance, c"key-press-event", on_key as *const (), hooks);
        for signal in [c"button-press-event", c"scroll-event", c"motion-notify-event"] {
            connect(instance, signal, on_pointer as *const (), hooks);
        }
    }
}

/// Connects `handler` to `signal` of `instance`, with a reference to `hooks`
/// as its data.
unsafe fn connect(instance: *mut c_void, signal: &CStr, handler: *const (), hooks: &Arc<Hooks>) {
    unsafe extern "C" fn release(data: *mut c_void, _closure: *mut gobject_ffi::GClosure) {
        // SAFETY: made by `Box::into_raw` below, released once.
        drop(unsafe { Box::from_raw(data.cast::<Arc<Hooks>>()) });
    }
    let data = Box::into_raw(Box::new(Arc::clone(hooks)));
    unsafe {
        gobject_ffi::g_signal_connect_data(
            instance.cast(),
            signal.as_ptr(),
            // SAFETY: a `GCallback` is any C function pointer, which the
            // signal calls with the arguments its handlers are written for.
            Some(std::mem::transmute::<*const (), unsafe extern "C" fn()>(handler)),
            data.cast(),
            Some(release),
            0,
        );
    }
}

/// `key-press-event`: Escape and F6 are consumed and told to the app, any
/// other key is activity and goes on to the page.
unsafe extern "C" fn on_key(
    _widget: Widget,
    event: *const KeyEvent,
    hooks: *const Arc<Hooks>,
) -> c_int {
    // SAFETY: GTK passes the event and the data given to `connect`.
    let (hooks, key) = unsafe { (&*hooks, (*event).keyval) };
    match key {
        KEY_ESCAPE => {
            hooks.escape();
            1
        }
        KEY_F6 => {
            hooks.focus_chrome();
            1
        }
        _ => {
            hooks.activity();
            0
        }
    }
}

/// `button-press-event`, `scroll-event` and `motion-notify-event`: activity,
/// and the event goes on to the page.
unsafe extern "C" fn on_pointer(
    _widget: Widget,
    _event: *const c_void,
    hooks: *const Arc<Hooks>,
) -> c_int {
    // SAFETY: GTK passes the data given to `connect`.
    unsafe { &*hooks }.activity();
    0
}

/// Moves the main web view into an overlay (once) and the surface over it.
///
/// SAFETY: `surface` is a live `GtkWidget`, and this is the GTK thread.
unsafe fn overlay(surface: Widget) {
    unsafe {
        let parent = gtk_widget_get_parent(surface);
        if parent.is_null() {
            return;
        }
        // Taking a focused widget out of its container drops the window's
        // focus, so the keys that follow (F6, the arrows) would reach nothing:
        // whichever widget had it gets it back at the end, except that a new
        // web view takes the focus as it is made, and the viewer's controls
        // (the main web view) are where it belongs. (`gtk_widget_has_focus`
        // is false in a window the system hasn't focused, so the window's own
        // record of its focus is read.)
        let focus = gtk_window_get_focus(gtk_widget_get_toplevel(surface));
        // The overlay a previous surface made, or the main web view to move.
        let (mut overlay, mut main): (Widget, Widget) = (ptr::null_mut(), ptr::null_mut());
        let list = gtk_container_get_children(parent);
        let mut node = list;
        while !node.is_null() {
            let child: Widget = (*node).data;
            if child != surface {
                if g_type_check_instance_is_a(
                    child.cast::<GObject>().cast(),
                    gtk_overlay_get_type(),
                ) != 0
                {
                    overlay = child;
                } else if main.is_null() {
                    main = child;
                }
            }
            node = (*node).next;
        }
        g_list_free(list);
        if overlay.is_null() {
            overlay = gtk_overlay_new();
            if !main.is_null() {
                g_object_ref(main.cast());
                gtk_container_remove(parent, main);
                gtk_container_add(overlay, main);
                g_object_unref(main.cast());
            }
            gtk_box_pack_start(parent, overlay, 1, 1, 0);
            gtk_widget_show_all(overlay);
        }
        g_object_ref(surface.cast());
        gtk_container_remove(parent, surface);
        gtk_overlay_add_overlay(overlay, surface);
        gtk_overlay_set_overlay_pass_through(overlay, surface, 0);
        g_object_unref(surface.cast());
        gtk_widget_set_halign(surface, ALIGN_START);
        gtk_widget_set_valign(surface, ALIGN_START);
        if !focus.is_null() {
            let main_view = if main.is_null() { gtk_bin_get_child(overlay) } else { main };
            let target = if focus == surface { main_view } else { focus };
            if !target.is_null() {
                gtk_widget_grab_focus(target);
            }
        }
    }
}

/// What the filter's save callback needs: the web view to add it to (held
/// until then) and where to say how it went.
struct FilterSave {
    view: *mut webkit2gtk::ffi::WebKitWebView,
    done: mpsc::Sender<Result<(), String>>,
}

/// Compiles `BLOCK_RULES` into the store at `store` and adds the result to
/// the web view's content manager; `done` is told how it went.
///
/// SAFETY: `view` is a live `WebKitWebView`, and this is the GTK thread.
unsafe fn install_filter(
    view: *mut webkit2gtk::ffi::WebKitWebView,
    store: &CStr,
    done: mpsc::Sender<Result<(), String>>,
) {
    let rules = glib::Bytes::from_owned(BLOCK_RULES.as_bytes().to_vec());
    let id = CString::new(FILTER_ID).expect("no NUL in the identifier");
    unsafe {
        let filter_store = webkit2gtk::ffi::webkit_user_content_filter_store_new(store.as_ptr());
        if filter_store.is_null() {
            let _ = done.send(Err("the content filter's store could not be made".into()));
            return;
        }
        gobject_ffi::g_object_ref(view.cast());
        let data = Box::into_raw(Box::new(FilterSave { view, done }));
        webkit2gtk::ffi::webkit_user_content_filter_store_save(
            filter_store,
            id.as_ptr(),
            rules.to_glib_none().0,
            ptr::null_mut(),
            Some(filter_saved),
            data.cast(),
        );
    }
}

/// The filter has been compiled: adds it to the web view, or says why not.
unsafe extern "C" fn filter_saved(
    store: *mut GObject,
    result: *mut webkit2gtk::gio::ffi::GAsyncResult,
    data: glib::ffi::gpointer,
) {
    // SAFETY: `data` is the box `install_filter` made, handed back once.
    let FilterSave { view, done } = *unsafe { Box::from_raw(data.cast::<FilterSave>()) };
    let outcome = unsafe {
        let mut error: *mut glib::ffi::GError = ptr::null_mut();
        let filter = webkit2gtk::ffi::webkit_user_content_filter_store_save_finish(
            store.cast(),
            result,
            &mut error,
        );
        if filter.is_null() {
            let message = if error.is_null() {
                "no reason given".to_owned()
            } else {
                let text = CStr::from_ptr((*error).message as *const c_char)
                    .to_string_lossy()
                    .into_owned();
                glib::ffi::g_error_free(error);
                text
            };
            Err(format!("the content filter could not be compiled: {message}"))
        } else {
            let manager = webkit2gtk::ffi::webkit_web_view_get_user_content_manager(view);
            webkit2gtk::ffi::webkit_user_content_manager_add_filter(manager, filter);
            webkit2gtk::ffi::webkit_user_content_filter_unref(filter);
            Ok(())
        }
    };
    unsafe {
        g_object_unref(view.cast());
        g_object_unref(store);
    }
    let _ = done.send(outcome);
}

/// Forces a layout and a paint of the page, whatever it shows.
const REPAINT: &str = "const d = document.documentElement; d.style.display = 'none'; \
    void d.offsetHeight; d.style.display = ''";

/// Places the surface over `bounds` and shows or hides it.
pub(super) fn place<R: Runtime>(
    webview: &Webview<R>,
    bounds: Rect,
    visible: bool,
) -> tauri::Result<()> {
    with_widget(webview, move |surface| unsafe {
        gtk_widget_set_margin_start(surface, bounds.x.round() as c_int);
        gtk_widget_set_margin_top(surface, bounds.y.round() as c_int);
        gtk_widget_set_size_request(
            surface,
            bounds.width.round() as c_int,
            bounds.height.round() as c_int,
        );
        gtk_widget_set_visible(surface, c_int::from(visible));
    })
}

/// Where the surface is now, as GTK has allocated it. Must not be called on
/// the main thread, which it waits on.
pub(super) fn bounds<R: Runtime>(webview: &Webview<R>) -> tauri::Result<Rect> {
    let (tx, rx) = mpsc::channel();
    with_widget(webview, move |surface| unsafe {
        let _ = tx.send(Rect {
            x: f64::from(gtk_widget_get_margin_start(surface)),
            y: f64::from(gtk_widget_get_margin_top(surface)),
            width: f64::from(gtk_widget_get_allocated_width(surface)),
            height: f64::from(gtk_widget_get_allocated_height(surface)),
        });
    })?;
    rx.recv().map_err(|_| tauri::Error::FailedToReceiveMessage)
}
