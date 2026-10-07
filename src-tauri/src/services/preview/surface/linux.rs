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

use std::{
    ffi::c_void,
    os::raw::{c_int, c_uint},
    sync::{Arc, mpsc},
};

use tauri::{Runtime, Url, Webview, Window, webview::WebviewBuilder};
use webkit2gtk::glib::{
    ffi::{GList, g_list_free},
    gobject_ffi::{GObject, g_object_ref, g_object_unref, g_type_check_instance_is_a},
    translate::ToGlibPtr,
};

use std::path::Path;

use super::{Hooks, Rect};

type Widget = *mut c_void;
const ALIGN_START: c_int = 1;

// libgtk-3 is already linked: wry's `gtk` crate links it.
unsafe extern "C" {
    fn gtk_widget_get_parent(widget: Widget) -> Widget;
    fn gtk_container_get_children(container: Widget) -> *mut GList;
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
pub(super) fn customize<R: Runtime>(
    builder: WebviewBuilder<R>,
    proxy_url: &Url,
    data_directory: &Path,
) -> WebviewBuilder<R> {
    builder.proxy_url(proxy_url.clone()).data_directory(data_directory.to_path_buf())
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

/// Moves the main web view into an overlay (once) and the surface over it.
/// Called once after `add_child`.
///
/// A web view that has been moved shows nothing until its page next changes,
/// so the other web views of the window are made to repaint.
pub(super) fn attach<R: Runtime>(
    window: &Window<R>,
    webview: &Webview<R>,
    _hooks: &Arc<Hooks>,
) -> tauri::Result<()> {
    with_widget(webview, |surface| unsafe {
        let parent = gtk_widget_get_parent(surface);
        if parent.is_null() {
            return;
        }
        // The overlay a previous surface made, or the main web view to move.
        let (mut overlay, mut main): (Widget, Widget) =
            (std::ptr::null_mut(), std::ptr::null_mut());
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
    })?;
    for other in window.webviews().iter().filter(|w| w.label() != super::LABEL) {
        other.eval(REPAINT)?;
    }
    Ok(())
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
