//! Linux: the native browser controls (008 research.md §13, §14, §15).
//!
//! WebKitGTK offers a context menu with Back, Forward, Reload, Inspect
//! Element, link and image items and more, none of which an application has.
//! `apply` connects the main web view's `context-menu` signal and keeps only
//! the stock actions that edit or copy text (008 FR-010, FR-011). It is an
//! allowlist: an action a later WebKitGTK adds is removed until someone
//! looks at it. Reload and navigation need nothing here: WebKitGTK binds no
//! key to them and GTK3 has no swipe navigation (008 research.md §14), so the
//! page's own handlers (`src/features/app/browserControls.ts`) are the
//! measure.

use tauri::{Runtime, WebviewWindow};
use webkit2gtk::{
    ContextMenuAction, ContextMenuExt, ContextMenuItem, ContextMenuItemExt, WebViewExt,
    gio::prelude::ActionExt,
};

/// Whether a menu item of this stock action is kept (008 FR-011). `custom_name`
/// is the name of the item's `GAction`, which is all a `Custom` item says
/// about itself.
fn keeps(action: &ContextMenuAction, custom_name: Option<&str>) -> bool {
    match action {
        ContextMenuAction::Cut
        | ContextMenuAction::Copy
        | ContextMenuAction::Paste
        | ContextMenuAction::Delete
        | ContextMenuAction::SelectAll
        | ContextMenuAction::InsertEmoji
        | ContextMenuAction::SpellingGuess
        | ContextMenuAction::NoGuessesFound
        | ContextMenuAction::IgnoreSpelling
        | ContextMenuAction::LearnSpelling
        | ContextMenuAction::IgnoreGrammar => true,
        // WebKit gives the actions it has no stock name for (some spelling
        // items among them) the `Custom` action. They are kept when the name
        // says spelling or grammar; the window controls check logs every
        // one it sees (008 research.md §13).
        ContextMenuAction::Custom => custom_name.is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name.contains("spell") || name.contains("grammar")
        }),
        _ => false,
    }
}

/// What the menu check logs about an item.
fn describe(item: &ContextMenuItem, custom_name: Option<&str>) -> String {
    if item.is_separator() {
        "separator".to_string()
    } else {
        format!("{:?} {:?}", item.stock_action(), custom_name.unwrap_or(""))
    }
}

/// Removes separators that lead, trail or repeat.
fn tidy_separators(menu: &webkit2gtk::ContextMenu) {
    let mut previous_was_separator = true;
    for item in menu.items() {
        if item.is_separator() {
            if previous_was_separator {
                menu.remove(&item);
            }
            previous_was_separator = true;
        } else {
            previous_was_separator = false;
        }
    }
    if let Some(last) = menu.items().last()
        && last.is_separator()
    {
        menu.remove(last);
    }
}

/// Trims the main web view's context menu to 008 FR-011's items. `log_items` logs
/// every item offered and whether it was kept, for the window controls check
/// (008 research.md §18). A menu left with nothing isn't shown.
///
/// Called on the `main` web view from `setup()` by `window_controls::apply`
/// (008 T080), in release and E2E builds only (008 research.md §15), on the main
/// thread, which `with_webview` runs it on.
pub(crate) fn apply<R: Runtime>(window: &WebviewWindow<R>, log_items: bool) -> tauri::Result<()> {
    window.with_webview(move |platform| {
        platform.inner().connect_context_menu(move |_, menu, _, _| {
            for item in menu.items() {
                let custom_name = item.gaction().map(|action| action.name().to_string());
                let keep =
                    !item.is_separator() && keeps(&item.stock_action(), custom_name.as_deref());
                if log_items {
                    log::info!(
                        "context menu item: {} ({})",
                        describe(&item, custom_name.as_deref()),
                        if keep || item.is_separator() { "kept" } else { "removed" }
                    );
                }
                if !keep && !item.is_separator() {
                    menu.remove(&item);
                }
            }
            tidy_separators(menu);
            // Nothing left to offer: no menu at all.
            menu.n_items() == 0
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_editing_and_spelling_actions() {
        for kept in [
            ContextMenuAction::Cut,
            ContextMenuAction::Copy,
            ContextMenuAction::Paste,
            ContextMenuAction::Delete,
            ContextMenuAction::SelectAll,
            ContextMenuAction::InsertEmoji,
            ContextMenuAction::SpellingGuess,
            ContextMenuAction::NoGuessesFound,
            ContextMenuAction::IgnoreSpelling,
            ContextMenuAction::LearnSpelling,
            ContextMenuAction::IgnoreGrammar,
        ] {
            assert!(keeps(&kept, None), "{kept:?}");
        }
        for removed in [
            ContextMenuAction::GoBack,
            ContextMenuAction::GoForward,
            ContextMenuAction::Stop,
            ContextMenuAction::Reload,
            ContextMenuAction::InspectElement,
            ContextMenuAction::OpenLink,
            ContextMenuAction::CopyLinkToClipboard,
            ContextMenuAction::CopyImageToClipboard,
            ContextMenuAction::DownloadImageToDisk,
            ContextMenuAction::InputMethods,
            ContextMenuAction::Unicode,
            ContextMenuAction::PasteAsPlainText,
            ContextMenuAction::NoAction,
        ] {
            assert!(!keeps(&removed, None), "{removed:?}");
        }
    }

    #[test]
    fn keeps_a_custom_item_only_if_it_is_about_spelling() {
        assert!(keeps(&ContextMenuAction::Custom, Some("Check-Spelling")));
        assert!(keeps(&ContextMenuAction::Custom, Some("grammar")));
        assert!(!keeps(&ContextMenuAction::Custom, Some("share")));
        assert!(!keeps(&ContextMenuAction::Custom, None));
    }
}
