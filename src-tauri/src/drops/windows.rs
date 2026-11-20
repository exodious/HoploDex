//! Windows: a drag that starts in a page is not taken as a drop.
//!
//! wry replaces WebView2's own drop target on each window of the web view
//! with one that reads a drop's files (`CF_HDROP`), whatever made the drag.
//! A drag out of a page is started by the WebView2 browser process, not this
//! one, so its source can't be asked; but Chromium marks the data of every
//! drag a renderer starts with its `chromium/x-renderer-taint` format, which
//! a drag from another app doesn't carry. [`refuse_app_drags`] puts a filter
//! in front of wry's drop target on each of those windows: a drag carrying
//! the mark is refused (no drop effect) and never reaches wry; every other
//! drag goes to wry's target unchanged.

use std::cell::Cell;

use tauri::{Runtime, WebviewWindow};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, POINTL},
        System::Threading::GetCurrentThreadId,
        System::{
            Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL},
            DataExchange::RegisterClipboardFormatW,
            Ole::{
                DROPEFFECT, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, RegisterDragDrop,
                RevokeDragDrop,
            },
            SystemServices::MODIFIERKEYS_FLAGS,
        },
        UI::WindowsAndMessaging::{EnumChildWindows, GetPropW, GetWindowThreadProcessId},
    },
    core::{BOOL, Interface, Ref, implement, w},
};

/// Puts the filter in front of wry's drop target on every window of the main
/// web view, on the main thread, and then lets drops be recorded. Fails, and
/// no drop is recorded, if wry's target is on none of them.
pub fn refuse_app_drags<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    let (done, result) = std::sync::mpsc::channel::<Result<usize, String>>();
    window.with_webview(move |platform| {
        let mut parent = HWND::default();
        // SAFETY: the controller's own window, asked on its thread, into a
        // local.
        let asked = unsafe { platform.controller().ParentWindow(&mut parent) };
        let _ = done.send(asked.map_err(|e| e.to_string()).map(|()| wrap_all(parent)));
    })?;
    match result.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(Ok(0)) => Err(std::io::Error::other("no drop target to filter").into()),
        Ok(Ok(_)) => {
            super::refusing();
            Ok(())
        }
        Ok(Err(why)) => Err(std::io::Error::other(why).into()),
        Err(_) => Err(std::io::Error::other("the drop filter was not set up in time").into()),
    }
}

/// Wraps the drop target of every window under `parent` (wry registers one on
/// each when it builds the web view) and returns how many it wrapped.
fn wrap_all(parent: HWND) -> usize {
    let mut wrapped = 0usize;
    unsafe extern "system" fn each(hwnd: HWND, count: LPARAM) -> BOOL {
        // SAFETY: `count` is the `usize` `wrap_all` passed, alive for the call.
        unsafe {
            if wrap(hwnd) {
                *(count.0 as *mut usize) += 1;
            }
        }
        true.into()
    }
    // SAFETY: the callback only reads the windows it is given.
    unsafe {
        let _ =
            EnumChildWindows(Some(parent), Some(each), LPARAM(&mut wrapped as *mut usize as isize));
    }
    wrapped
}

/// Replaces `hwnd`'s registered drop target with the filter in front of it,
/// if `hwnd` is a window of this thread (the enumeration also lists the
/// WebView2 processes' windows, whose property means nothing here).
///
/// # Safety
/// `hwnd` is a live window handle.
unsafe fn wrap(hwnd: HWND) -> bool {
    // SAFETY: plain queries of a window handle and of this thread.
    if unsafe { GetWindowThreadProcessId(hwnd, None) != GetCurrentThreadId() } {
        return false;
    }
    // SAFETY: OLE keeps the target `RegisterDragDrop` was given in this
    // property, holding a reference, until `RevokeDragDrop`; the clone takes
    // a reference of its own before that.
    unsafe {
        let raw = GetPropW(hwnd, w!("OleDropTargetInterface"));
        if raw.0.is_null() {
            return false;
        }
        let Some(inner) = IDropTarget::from_raw_borrowed(&raw.0).cloned() else {
            return false;
        };
        if RevokeDragDrop(hwnd).is_err() {
            return false;
        }
        let filter: IDropTarget =
            AppDragFilter { inner: inner.clone(), refused: Cell::new(false) }.into();
        if RegisterDragDrop(hwnd, &filter).is_err() {
            // Put wry's back rather than leave the window without one.
            let _ = RegisterDragDrop(hwnd, &inner);
            return false;
        }
        true
    }
}

/// Whether Chromium marked `data` as made by a renderer: a drag out of a page.
fn from_a_page(data: Option<&IDataObject>) -> bool {
    let Some(data) = data else { return false };
    // SAFETY: registering a format name and asking whether the data has it.
    unsafe {
        let format = RegisterClipboardFormatW(w!("chromium/x-renderer-taint"));
        if format == 0 {
            // Without the format's number, take every drag as the page's.
            return true;
        }
        let query = FORMATETC {
            cfFormat: format as u16,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        data.QueryGetData(&query).is_ok()
    }
}

#[implement(IDropTarget)]
struct AppDragFilter {
    inner: IDropTarget,
    /// Whether the drag now over the window is a page's.
    refused: Cell<bool>,
}

impl IDropTarget_Impl for AppDragFilter_Impl {
    fn DragEnter(
        &self,
        data: Ref<'_, IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let refused = from_a_page(data.as_ref());
        self.refused.set(refused);
        if refused {
            log::info!("a drag that started in a page was not taken as a drop");
            // SAFETY: OLE's out-parameter.
            unsafe { *effect = DROPEFFECT_NONE };
            return Ok(());
        }
        // SAFETY: forwarding OLE's own arguments.
        unsafe { self.inner.DragEnter(data.as_ref(), keys, *point, effect) }
    }

    fn DragOver(
        &self,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        if self.refused.get() {
            // SAFETY: OLE's out-parameter.
            unsafe { *effect = DROPEFFECT_NONE };
            return Ok(());
        }
        // SAFETY: forwarding OLE's own arguments.
        unsafe { self.inner.DragOver(keys, *point, effect) }
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        if self.refused.replace(false) {
            return Ok(());
        }
        // SAFETY: a plain call on wry's target.
        unsafe { self.inner.DragLeave() }
    }

    fn Drop(
        &self,
        data: Ref<'_, IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        // A page's drag is refused at the drop too, whatever its entry said.
        if self.refused.replace(false) || from_a_page(data.as_ref()) {
            // SAFETY: OLE's out-parameter.
            unsafe { *effect = DROPEFFECT_NONE };
            return Ok(());
        }
        // SAFETY: forwarding OLE's own arguments.
        unsafe { self.inner.Drop(data.as_ref(), keys, *point, effect) }
    }
}
