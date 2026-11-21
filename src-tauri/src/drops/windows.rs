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
        Foundation::{HWND, LPARAM, POINTL, S_OK},
        System::{
            Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL},
            DataExchange::RegisterClipboardFormatW,
            LibraryLoader::GetModuleHandleW,
            Memory::{
                MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
                PAGE_GUARD, PAGE_READONLY, PAGE_READWRITE, VirtualQuery,
            },
            Ole::{
                DROPEFFECT, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, RegisterDragDrop,
                RevokeDragDrop,
            },
            SystemServices::MODIFIERKEYS_FLAGS,
        },
        UI::WindowsAndMessaging::{EnumChildWindows, GetPropW},
    },
    core::{BOOL, Interface, Ref, implement, w},
};

/// Puts the filter in front of wry's drop target on every window of the main
/// web view, on the main thread, and then lets drops be recorded. Fails, and
/// no drop is recorded, if wry's target is on none of them, or if one of
/// them couldn't be wrapped (that window would take drops unfiltered).
pub fn refuse_app_drags<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    let (done, result) = std::sync::mpsc::channel::<Result<Wrapped, String>>();
    window.with_webview(move |platform| {
        let mut parent = HWND::default();
        // SAFETY: the controller's own window, asked on its thread, into a
        // local.
        let asked = unsafe { platform.controller().ParentWindow(&mut parent) };
        let _ = done.send(asked.map_err(|e| e.to_string()).map(|()| wrap_all(parent)));
    })?;
    match result.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(Ok(Wrapped { failed, .. })) if failed > 0 => {
            Err(std::io::Error::other("a drop target couldn't be filtered").into())
        }
        Ok(Ok(Wrapped { wrapped: 0, .. })) => {
            Err(std::io::Error::other("no drop target to filter").into())
        }
        Ok(Ok(_)) => {
            super::refusing();
            Ok(())
        }
        Ok(Err(why)) => Err(std::io::Error::other(why).into()),
        Err(_) => Err(std::io::Error::other("the drop filter was not set up in time").into()),
    }
}

/// How many of wry's drop targets were wrapped, and how many couldn't be.
#[derive(Default)]
struct Wrapped {
    wrapped: usize,
    failed: usize,
}

/// Wraps the drop target of every window under `parent` (wry registers one on
/// each when it builds the web view).
fn wrap_all(parent: HWND) -> Wrapped {
    let mut counts = Wrapped::default();
    unsafe extern "system" fn each(hwnd: HWND, counts: LPARAM) -> BOOL {
        // SAFETY: `counts` is the `Wrapped` `wrap_all` passed, alive for the
        // call.
        unsafe {
            let counts = &mut *(counts.0 as *mut Wrapped);
            match wrap(hwnd) {
                Some(true) => counts.wrapped += 1,
                Some(false) => counts.failed += 1,
                None => {}
            }
        }
        true.into()
    }
    // SAFETY: the callback only reads the windows it is given.
    unsafe {
        let _ = EnumChildWindows(
            Some(parent),
            Some(each),
            LPARAM(&mut counts as *mut Wrapped as isize),
        );
    }
    counts
}

/// Replaces `hwnd`'s registered drop target with the filter in front of it,
/// if the target is wry's: `None` when it isn't (or there is none), else
/// whether the filter is in place. wry registers on WebView2's windows, which its
/// browser and GPU processes own, so neither the window's process nor its
/// thread says whose the property's pointer is; [`ours`] asks the pointer.
///
/// # Safety
/// `hwnd` is a live window handle.
unsafe fn wrap(hwnd: HWND) -> Option<bool> {
    // SAFETY: OLE keeps the target `RegisterDragDrop` was given in this
    // property, holding a reference, until `RevokeDragDrop`; the clone takes
    // a reference of its own before that.
    unsafe {
        let raw = GetPropW(hwnd, w!("OleDropTargetInterface"));
        if raw.0.is_null() || !ours(raw.0) {
            return None;
        }
        let inner = IDropTarget::from_raw_borrowed(&raw.0).cloned()?;
        if RevokeDragDrop(hwnd).is_err() {
            return Some(false);
        }
        let filter: IDropTarget =
            AppDragFilter { inner: inner.clone(), refused: Cell::new(false) }.into();
        if RegisterDragDrop(hwnd, &filter).is_err() {
            // Put wry's back rather than leave the window without one.
            let _ = RegisterDragDrop(hwnd, &inner);
            return Some(false);
        }
        Some(true)
    }
}

/// Whether `object` is a COM object this executable made, as wry's drop
/// targets are: committed, readable memory of this process whose first word,
/// the vtable, lies in this executable's image. A pointer another process
/// stored in a window property fails it, and is never followed.
fn ours(object: *mut std::ffi::c_void) -> bool {
    let readable = |address: *const std::ffi::c_void| {
        let mut info = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: VirtualQuery only describes the address, into a local.
        let size = unsafe {
            VirtualQuery(Some(address), &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>())
        };
        let readable = PAGE_READONLY | PAGE_READWRITE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE;
        (size != 0
            && info.State == MEM_COMMIT
            && (info.Protect & readable).0 != 0
            && (info.Protect & PAGE_GUARD).0 == 0)
            .then_some(info)
    };
    if object.is_null() || !(object as usize).is_multiple_of(std::mem::align_of::<usize>()) {
        return false;
    }
    // SAFETY: a module handle for this executable, which isn't freed.
    let Ok(image) = (unsafe { GetModuleHandleW(None) }) else { return false };
    if readable(object).is_none() {
        return false;
    }
    // SAFETY: an aligned word in committed, readable memory of this process
    // (an aligned word never crosses a page).
    let vtable = unsafe { *(object as *const *const std::ffi::c_void) };
    readable(vtable).is_some_and(|info| info.AllocationBase == image.0)
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
        // Only S_OK says the data has the format. File Explorer's data
        // object answers S_FALSE, a success code, for a format it lacks
        // (#207).
        data.QueryGetData(&query) == S_OK
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

#[cfg(test)]
mod tests {
    use windows::Win32::{
        Foundation::HGLOBAL,
        System::{
            Com::{STGMEDIUM, STGMEDIUM_0},
            Memory::{GMEM_MOVEABLE, GlobalAlloc},
            Ole::OleInitialize,
        },
        UI::Shell::SHCreateDataObject,
    };

    use super::*;

    /// The shell's own data object, the kind File Explorer drags.
    fn shell_data_object() -> IDataObject {
        // SAFETY: OLE on this thread, then an empty data object.
        unsafe {
            let _ = OleInitialize(None);
            SHCreateDataObject(None, None, None::<&IDataObject>).expect("SHCreateDataObject")
        }
    }

    // A drag from File Explorer is a drop of the user's files: its data
    // object answers S_FALSE, not an error, for the mark it doesn't have
    // (#207).
    #[test]
    fn a_drag_from_explorer_is_not_a_pages() {
        let data = shell_data_object();
        let format = unsafe { RegisterClipboardFormatW(w!("chromium/x-renderer-taint")) };
        let query = FORMATETC {
            cfFormat: format as u16,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        // SAFETY: asking a live data object about a format.
        let answer = unsafe { data.QueryGetData(&query) };
        assert!(answer.is_ok() && answer != S_OK, "the shell answers {answer:?}");
        assert!(!from_a_page(Some(&data)));
    }

    #[test]
    fn a_drag_carrying_the_mark_is_a_pages() {
        let data = shell_data_object();
        let format = unsafe { RegisterClipboardFormatW(w!("chromium/x-renderer-taint")) };
        let query = FORMATETC {
            cfFormat: format as u16,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        // SAFETY: a one-byte block handed to the data object, which frees it
        // (`fRelease`).
        unsafe {
            let block: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, 1).unwrap();
            let medium = STGMEDIUM {
                tymed: TYMED_HGLOBAL.0 as u32,
                u: STGMEDIUM_0 { hGlobal: block },
                pUnkForRelease: std::mem::ManuallyDrop::new(None),
            };
            data.SetData(&query, &medium, true).unwrap();
        }
        assert!(from_a_page(Some(&data)));
    }

    #[test]
    fn no_data_is_not_a_pages() {
        assert!(!from_a_page(None));
    }
}
