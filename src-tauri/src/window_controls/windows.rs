//! Windows: the native browser controls (008 research.md §13, §14, §15).
//!
//! `apply` reaches the main web view's COM interfaces through Tauri's
//! `with_webview` (`webview2-com`, the crate wry uses), on the main thread,
//! and does three things:
//! - `ICoreWebView2_11::add_ContextMenuRequested` keeps the menu items named
//!   in [`KEPT`] and removes every other name, so an item a later WebView2
//!   adds is gone until someone looks at it. A menu left empty sets
//!   `Handled`, so none appears;
//! - `ICoreWebView2Settings3::put_AreBrowserAcceleratorKeysEnabled(false)`
//!   stops F5, Ctrl+R, Ctrl+P and the other browser accelerators acting in
//!   WebView2 itself (the editing shortcuts aren't browser accelerators);
//! - `ICoreWebView2Settings6::put_IsSwipeNavigationEnabled(false)` stops the
//!   history swipe on a touchpad.
//!
//! This adds no `WebResourceRequested` filter or handler: the main view's
//! document filter (`services/main_navigation.rs`) is the only one, and it
//! must keep seeing document requests only (008 research.md §2).

use ::windows::core::{Interface, PWSTR};
use tauri::{Runtime, WebviewWindow};
use webview2_com::{
    ContextMenuRequestedEventHandler, Microsoft::Web::WebView2::Win32::*, take_pwstr,
};

/// The items 008 FR-011 keeps, by WebView2's `Name`. Spelling suggestions arrive
/// as `COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND` items named
/// `spellcheck` (lower case, as the check logged them; 008 T085).
const KEPT: &[&str] = &["cut", "copy", "paste", "selectAll", "undo", "redo", "emoji", "spellcheck"];

/// Whether a menu item is kept, by name. Separators are handled apart.
fn is_kept(name: &str) -> bool {
    KEPT.contains(&name)
}

/// What a `PWSTR`-returning getter gave, owned (empty on failure).
fn text(f: impl FnOnce(&mut PWSTR) -> ::windows::core::Result<()>) -> String {
    let mut p = PWSTR::null();
    if f(&mut p).is_err() {
        return String::new();
    }
    take_pwstr(p)
}

/// Trims the menu to 008 FR-011's items, logging each one offered when asked.
fn filter_menu(
    args: &ICoreWebView2ContextMenuRequestedEventArgs,
    log_items: bool,
) -> ::windows::core::Result<()> {
    let mut count = 0u32;
    // SAFETY: COM calls on the event's own objects, on the thread that owns
    // them; each out-parameter is a local that outlives its call.
    unsafe {
        let items = args.MenuItems()?;
        items.Count(&mut count)?;
        // Backwards, so a removal doesn't move an index still to visit.
        for index in (0..count).rev() {
            let item = items.GetValueAtIndex(index)?;
            let mut kind = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND::default();
            item.Kind(&mut kind)?;
            if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR {
                continue;
            }
            let name = text(|p| item.Name(p));
            let keep = is_kept(&name);
            if log_items {
                let label = text(|p| item.Label(p));
                log::info!(
                    "menu item name={name} kind={} label={label:?} -> {}",
                    kind.0,
                    if keep { "kept" } else { "removed" }
                );
            }
            if !keep {
                items.RemoveValueAtIndex(index)?;
            }
        }
        tidy_separators(&items)?;
        items.Count(&mut count)?;
        if count == 0 {
            // Nothing to show, so no menu.
            args.SetHandled(true)?;
        }
    }
    Ok(())
}

/// Removes the separators the filter left leading, trailing or doubled, so a
/// menu of what's kept has none of the removed items' dividers.
///
/// # Safety
/// `items` must be live and used on its own thread.
unsafe fn tidy_separators(
    items: &ICoreWebView2ContextMenuItemCollection,
) -> ::windows::core::Result<()> {
    // SAFETY: the caller's contract.
    unsafe {
        let mut count = 0u32;
        items.Count(&mut count)?;
        let is_separator = |index: u32| -> ::windows::core::Result<bool> {
            let item = items.GetValueAtIndex(index)?;
            let mut kind = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND::default();
            item.Kind(&mut kind)?;
            Ok(kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR)
        };
        let mut previous_is_separator = true; // a leading separator goes
        let mut index = 0u32;
        while index < count {
            let separator = is_separator(index)?;
            if separator && previous_is_separator {
                items.RemoveValueAtIndex(index)?;
                count -= 1;
                continue;
            }
            previous_is_separator = separator;
            index += 1;
        }
        if count > 0 && is_separator(count - 1)? {
            items.RemoveValueAtIndex(count - 1)?;
        }
    }
    Ok(())
}

/// Sets the main web view up, on the main thread. Every interface is queried
/// before anything is set, so a runtime that lacks one changes nothing.
fn configure(platform: &tauri::webview::PlatformWebview, log_items: bool) -> Result<(), String> {
    let err = |e: ::windows::core::Error| e.to_string();
    let missing = |name: &'static str| {
        move |e: ::windows::core::Error| {
            format!("this computer's WebView2 runtime has no {name} ({e})")
        }
    };
    // SAFETY: COM calls on the web view's own objects, made on the thread
    // that owns them; each out-parameter is a local that outlives its call.
    unsafe {
        let webview = platform.controller().CoreWebView2().map_err(err)?;
        let settings = webview.Settings().map_err(err)?;
        let settings3: ICoreWebView2Settings3 =
            settings.cast().map_err(missing("ICoreWebView2Settings3"))?;
        let settings6: ICoreWebView2Settings6 =
            settings.cast().map_err(missing("ICoreWebView2Settings6"))?;
        let webview11: ICoreWebView2_11 = webview.cast().map_err(missing("ICoreWebView2_11"))?;

        settings3.SetAreBrowserAcceleratorKeysEnabled(false).map_err(err)?;
        settings6.SetIsSwipeNavigationEnabled(false).map_err(err)?;

        let mut token = 0i64;
        webview11
            .add_ContextMenuRequested(
                &ContextMenuRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    filter_menu(&args, log_items)
                })),
                &mut token,
            )
            .map_err(err)?;
    }
    Ok(())
}

/// Installs the context menu filter, and turns the browser accelerator keys
/// and swipe navigation off, on the main web view. `log_items` logs each
/// menu item offered as `menu item name=<name> ... -> kept|removed`, which
/// the window controls check parses (008 research.md §18).
pub(crate) fn apply<R: Runtime>(window: &WebviewWindow<R>, log_items: bool) -> tauri::Result<()> {
    let (done, result) = std::sync::mpsc::channel::<Result<(), String>>();
    window.with_webview(move |platform| {
        let _ = done.send(configure(&platform, log_items));
    })?;
    match result.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(why)) => Err(std::io::Error::other(why).into()),
        Err(_) => Err(std::io::Error::other("the web view was not set up in time").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::is_kept;

    #[test]
    fn keeps_editing_emoji_and_spelling_items_only() {
        for name in ["cut", "copy", "paste", "selectAll", "undo", "redo", "emoji", "spellcheck"] {
            assert!(is_kept(name), "{name}");
        }
        for name in [
            "back",
            "forward",
            "reload",
            "saveAs",
            "print",
            "share",
            "webCapture",
            "inspectElement",
            "copyLinkToHighlight",
            "openLinkInNewWindow",
            "pasteAsPlainText",
            "",
        ] {
            assert!(!is_kept(name), "{name}");
        }
    }
}
