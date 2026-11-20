//! Linux: a drag that starts in the app carries no file paths.
//!
//! WebKitGTK reads a dropped `text/uri-list` as files, and wry hands Tauri
//! whatever URIs a drag carries. GTK knows a drag's source widget only when it
//! is in this process, so every drag with one started in the app: in the main
//! web view's page, or in another web view such as the PDF surface.
//! An emission hook on `drag-data-received` runs before every handler, wry's
//! included, and turns the URI list such a drag delivers into comments, so it
//! names no file when wry reads it.

use std::sync::Once;

use gtk::glib::gobject_ffi;

/// Installs the hook, once per process, and then lets drops be recorded.
pub fn refuse_app_drags() -> Result<(), String> {
    static HOOK: Once = Once::new();
    let mut result = Err("installed already".to_string());
    HOOK.call_once(|| {
        // SAFETY: GTK is initialised (a window exists) and this runs on its
        // thread; the signal is GtkWidget's own, and the hook lives for the
        // process.
        unsafe {
            let id = gobject_ffi::g_signal_lookup(
                c"drag-data-received".as_ptr(),
                gtk::ffi::gtk_widget_get_type(),
            );
            if id == 0 {
                result = Err("GtkWidget has no drag-data-received signal".into());
                return;
            }
            gobject_ffi::g_signal_add_emission_hook(
                id,
                0,
                Some(empty_app_drag),
                std::ptr::null_mut(),
                None,
            );
        }
        super::refusing();
        result = Ok(());
    });
    result
}

/// `drag-data-received(widget, context, x, y, data, info, time)`: empties
/// `data` when `context`'s drag started in this process.
unsafe extern "C" fn empty_app_drag(
    _hint: *mut gobject_ffi::GSignalInvocationHint,
    count: u32,
    values: *const gobject_ffi::GValue,
    _data: gtk::glib::ffi::gpointer,
) -> gtk::glib::ffi::gboolean {
    if count >= 5 && !values.is_null() {
        // SAFETY: GLib passes the emission's `count` parameter values, which
        // for this signal are the widget, the drag context and the selection
        // data among them.
        unsafe {
            let values = std::slice::from_raw_parts(values, count as usize);
            let context: *mut gtk::gdk::ffi::GdkDragContext =
                gobject_ffi::g_value_get_object(&values[1]).cast();
            let data: *mut gtk::ffi::GtkSelectionData =
                gobject_ffi::g_value_get_boxed(&values[4]).cast();
            if !context.is_null()
                && !data.is_null()
                && !gtk::ffi::gtk_drag_get_source_widget(context).is_null()
                && blank_uri_list(data)
            {
                log::info!("a drag that started in the app was not taken as a drop");
            }
        }
    }
    // Keep the hook for the next emission.
    gtk::glib::ffi::GTRUE
}

/// Overwrites a `text/uri-list`'s bytes with `#`, and says whether it did: every line a comment, so
/// `gtk_selection_data_get_uris` finds none. In place, because GTK frees the
/// buffer after the emission: replacing it (`gtk_selection_data_set`) frees
/// it twice.
///
/// # Safety
///
/// `data` is the selection data of a `drag-data-received` emission in
/// progress, whose buffer GTK allocated and owns for the emission.
unsafe fn blank_uri_list(data: *mut gtk::ffi::GtkSelectionData) -> bool {
    // SAFETY: as the caller promises; `length` bytes from `bytes` are the
    // buffer's (GTK adds a terminating NUL past them).
    unsafe {
        let uri_list = gtk::gdk::ffi::gdk_atom_intern_static_string(c"text/uri-list".as_ptr());
        if gtk::ffi::gtk_selection_data_get_data_type(data) != uri_list {
            return false;
        }
        let length = gtk::ffi::gtk_selection_data_get_length(data);
        let bytes = gtk::ffi::gtk_selection_data_get_data(data).cast_mut();
        if let Ok(length) = usize::try_from(length)
            && !bytes.is_null()
        {
            std::ptr::write_bytes(bytes, b'#', length);
            return true;
        }
        false
    }
}
