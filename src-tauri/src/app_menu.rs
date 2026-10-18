//! The macOS application menu: Tauri's default menu with the predefined About
//! item replaced by one that opens HoploDex's own About screen (010
//! research.md §11, contracts/tauri-commands.md). Linux and Windows set no
//! application menu; the id and event names and the mapping between them are
//! plain data, so they compile and are tested on every OS.

/// The id of the application menu's "About HoploDex" item (010 FR-001).
pub const ABOUT_MENU_ID: &str = "about";

/// What the frontend is sent when that item is chosen: `SessionProvider`
/// shows About HoploDex, as it asks about unsaved changes for
/// `app:quit-requested`. Must equal the name `sessionService.ts` listens for.
pub const ABOUT_REQUESTED: &str = "app:about-requested";

/// The event a chosen menu item asks the frontend for, or none for an item
/// the app doesn't handle itself (the predefined ones act on their own).
pub fn requested_event(menu_id: &str) -> Option<&'static str> {
    (menu_id == ABOUT_MENU_ID).then_some(ABOUT_REQUESTED)
}

/// Tauri's default macOS menu, with the application submenu's predefined
/// About item replaced, at the same position, by an item of id
/// [`ABOUT_MENU_ID`]. Every other item is left as Tauri builds it.
#[cfg(target_os = "macos")]
pub fn build<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{Menu, MenuItem, MenuItemKind};

    /// Where `Menu::default` puts About: first in the first submenu.
    const ABOUT_POSITION: usize = 0;

    let menu = Menu::default(app)?;
    let Some(application) =
        menu.items()?.into_iter().next().and_then(|kind| kind.as_submenu().cloned())
    else {
        log::warn!("the default menu has no application submenu; left as is");
        return Ok(menu);
    };
    let about = MenuItem::with_id(app, ABOUT_MENU_ID, "About HoploDex", true, None::<&str>)?;
    match application.remove_at(ABOUT_POSITION)? {
        Some(MenuItemKind::Predefined(_)) => application.insert(&about, ABOUT_POSITION)?,
        // Not the predefined About (a Tauri update moved things): put back
        // what was taken out and leave the default menu as it is.
        Some(other) => {
            application.insert(&other, ABOUT_POSITION)?;
            log::warn!("the default menu's first item is not the predefined About; left as is");
        }
        None => log::warn!("the default menu's application submenu is empty; left as is"),
    }
    Ok(menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 010 US1-7: the application menu's item id and the event it raises are
    // fixed by 010 contracts/tauri-commands.md; this module compiles on every OS.
    #[test]
    fn about_item_and_event_names_are_the_contracts() {
        assert_eq!(ABOUT_MENU_ID, "about");
        assert_eq!(ABOUT_REQUESTED, "app:about-requested");
    }

    #[test]
    fn only_the_about_item_raises_the_event() {
        assert_eq!(requested_event("about"), Some(ABOUT_REQUESTED));
        assert_eq!(requested_event("quit"), None);
        assert_eq!(requested_event(""), None);
        assert_eq!(requested_event("About"), None);
    }

    // 010 US1-7: the frontend listens for the same event name, so the two
    // sides can't drift apart.
    #[test]
    fn the_frontend_listens_for_the_same_event() {
        let service = include_str!("../../src/features/session/sessionService.ts");
        let listening = format!("listen<unknown>(\"{ABOUT_REQUESTED}\"");
        assert!(service.contains(&listening), "sessionService.ts must contain {listening}");
    }
}
