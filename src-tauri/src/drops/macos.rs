//! macOS: a drag that starts in the app is not taken as a drop.
//!
//! wry's `WryWebView` reads a drop's files from the pasteboard's file names in
//! `performDragOperation:`, whatever made the drag. AppKit names a drag's
//! source (`NSDraggingInfo.draggingSource`) only when it is in this process:
//! the page's own drag (WebKit starts it from the web view) or one from
//! another of the app's views. [`refuse_app_drags`] replaces the class's
//! `performDragOperation:` with one that refuses such a drag, telling wry the
//! drag left (`draggingExited:`) instead, and hands every other drag to wry's.
//! The class is shared by every wry web view, the PDF surface's included.

use std::{
    ffi::c_void,
    sync::{Once, OnceLock},
};

use objc2::{
    ffi::{class_getInstanceMethod, method_setImplementation},
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Bool, Imp, Sel},
    sel,
};
use tauri::{Runtime, WebviewWindow};

/// Installs the replacement on the main web view's class, once per process,
/// on the main thread, and then lets drops be recorded. Until it is in place
/// (or if it can't be) no drop is recorded.
pub fn refuse_app_drags<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    window.with_webview(|platform| {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| match install(platform.inner()) {
            Ok(()) => super::refusing(),
            Err(err) => {
                log::error!("drags from the app can't be refused, so no drop is recorded: {err}")
            }
        });
    })
}

/// wry's `performDragOperation:`, which every drag from outside the app goes to.
static WRY_PERFORM: OnceLock<usize> = OnceLock::new();

fn install(web_view: *mut c_void) -> Result<(), String> {
    if web_view.is_null() {
        return Err("no web view".into());
    }
    // SAFETY: `PlatformWebview::inner` is a live WryWebView.
    let object: &AnyObject = unsafe { &*web_view.cast::<AnyObject>() };
    let class: &AnyClass = crate::window_controls::wry_web_view_class(object.class())
        .ok_or("the web view isn't a WryWebView")?;
    // SAFETY: the method is WryWebView's own (wry defines it) and the
    // replacement has its signature, `B@:@`.
    unsafe {
        let method = class_getInstanceMethod(class, sel!(performDragOperation:));
        if method.is_null() {
            return Err("WryWebView has no performDragOperation:".into());
        }
        let replacement = std::mem::transmute::<
            unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject) -> Bool,
            Imp,
        >(perform_drag_operation);
        // Set before the swap, so the replacement always finds wry's.
        let wry = (*method).implementation();
        WRY_PERFORM.set(wry as usize).map_err(|_| "installed twice")?;
        method_setImplementation(method.cast_mut(), replacement);
    }
    Ok(())
}

/// `-[WryWebView performDragOperation:]`.
unsafe extern "C-unwind" fn perform_drag_operation(
    this: &AnyObject,
    cmd: Sel,
    info: &AnyObject,
) -> Bool {
    // SAFETY: NSDraggingInfo's public property: the drag's source when it is
    // in this application, nil otherwise.
    let source: Option<Retained<AnyObject>> = unsafe { msg_send![info, draggingSource] };
    if source.is_some() {
        log::info!("a drag that started in the app was not taken as a drop");
        // SAFETY: WryWebView's `draggingExited:` tells the page the drag left.
        let () = unsafe { msg_send![this, draggingExited: info] };
        return Bool::NO;
    }
    let Some(&wry) = WRY_PERFORM.get() else {
        return Bool::NO;
    };
    // SAFETY: wry's implementation, of this signature.
    let wry = unsafe {
        std::mem::transmute::<usize, unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject) -> Bool>(
            wry,
        )
    };
    // SAFETY: as AppKit called this one.
    unsafe { wry(this, cmd, info) }
}
