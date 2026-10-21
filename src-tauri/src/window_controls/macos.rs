//! macOS: the native browser controls (008 research.md §13, §14, §15).
//!
//! Tauri's web view on macOS is wry's `WryWebView`, a `WKWebView` subclass.
//! [`apply`] adds three methods to that class, once, after the main window's
//! web view exists (the class is registered when the first one is made):
//!
//! - `willOpenMenu:withEvent:`, the public `NSView` hook AppKit calls with
//!   the context menu before it is shown. The hook calls the superclass first
//!   (WebKit builds its items there), then removes every item that is not
//!   editing or copying: an allowlist, so an item a later macOS adds is
//!   removed until someone looks at it. It also turns off the menu's plug-in
//!   items (`allowsContextMenuPlugIns`). It logs every item it saw and what
//!   it did (`log::info!`).
//! - `validRequestorForSendType:returnType:`, returning nil: no Services menu.
//!   AppKit adds Services after the hook has run, so the menu's items can't
//!   remove it.
//! - `quickLookWithEvent:`, NSResponder's method for the three-finger-tap
//!   Look Up, doing nothing. Checked in the macOS 26 VM: it is also the entry
//!   of ⌃⌘D's Look Up, which the page never sees as a key event.
//!
//! The kept items are the ones 008 research.md §13 names: Cut, Copy, Paste, the
//! Spelling and Grammar menu (with Show Spelling and Grammar and Check
//! Spelling While Typing), the spelling guesses, Ignore and Learn Spelling,
//! and AppKit's Emoji & Symbols. AppKit also adds AutoFill to every context
//! menu after the hook, in a way no public API removes. The owner chose to
//! keep it in the editing and copying menus (008 spec.md Clarifications
//! 2026-10-11), and where the filter leaves no item the hook has the menu
//! cancelled (`cancelTrackingWithoutAnimation`, once it is up), so a blank
//! area, a button or an image opens none (008 research.md §13). The page's
//! `webkitmouseforcewillbegin` handling (008 research.md §12) covers the force click's Look
//! Up.
//!
//! The class is shared by every wry web view, so the PDF surface's child web
//! view, made after the main one, gets the same menu. The surface cancels
//! `contextmenu` in its page (`surface/frame_script.js`), so nothing is lost.

use std::{
    ffi::{CStr, c_char, c_void},
    sync::{
        Once,
        atomic::{AtomicBool, Ordering},
    },
};

use objc2::{
    ffi::class_addMethod,
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Imp, Sel},
    sel,
};
use objc2_foundation::{NSArray, NSString};
use tauri::{Runtime, WebviewWindow};

/// `WKMenuItemIdentifier…` names (the `identifier` WebKit gives each item)
/// that are kept (008 research.md §13).
const KEPT_IDENTIFIERS: &[&str] = &[
    "WKMenuItemIdentifierCut",
    "WKMenuItemIdentifierCopy",
    "WKMenuItemIdentifierPaste",
    "WKMenuItemIdentifierSelectAll",
    "WKMenuItemIdentifierSpellingMenu",
    "WKMenuItemIdentifierSpellingGuess",
    "WKMenuItemIdentifierIgnoreSpelling",
    "WKMenuItemIdentifierLearnSpelling",
];

/// The action of a Select All item that arrives without WebKit's identifier
/// (008 FR-011 offers select all where the system does).
const SELECT_ALL_ACTION: &str = "selectAll:";

/// The Spelling and Grammar menu's two kept items.
const KEPT_SPELLING_IDENTIFIERS: &[&str] =
    &["WKMenuItemIdentifierShowSpellingPanel", "WKMenuItemIdentifierCheckSpellingWhileTyping"];

/// Items WebKit gives no `WKMenuItemIdentifier…` (checked on macOS 26: their
/// `identifier` is AppKit's default, the action's name), found by their `tag`,
/// WebKit's `ContextMenuAction`, with the action they all share.
const TAGGED_ACTION: &str = "forwardContextMenuAction:";
const CUT_TAG: isize = 13;
const SPELLING_GUESS_TAG: isize = 15;
const IGNORE_SPELLING_TAG: isize = 17;
const LEARN_SPELLING_TAG: isize = 18;
const KEPT_TAGS: &[isize] = &[CUT_TAG, SPELLING_GUESS_TAG, IGNORE_SPELLING_TAG, LEARN_SPELLING_TAG];

/// AppKit's Emoji & Symbols (it has no identifier of WebKit's).
const EMOJI_ACTION: &str = "orderFrontCharacterPalette:";

/// The identifier of the item holding the spelling submenu.
const SPELLING_MENU: &str = "WKMenuItemIdentifierSpellingMenu";

/// What the filter knows about one menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    identifier: Option<String>,
    action: Option<String>,
    /// WebKit's `ContextMenuAction`, the item's `tag`.
    tag: isize,
    separator: bool,
}

/// Whether a top-level (non-separator) item is kept.
fn keep_top_level(item: &Item) -> bool {
    item.identifier.as_deref().is_some_and(|id| KEPT_IDENTIFIERS.contains(&id))
        || item.action.as_deref() == Some(EMOJI_ACTION)
        || item.action.as_deref() == Some(SELECT_ALL_ACTION)
        || (item.action.as_deref() == Some(TAGGED_ACTION) && KEPT_TAGS.contains(&item.tag))
}

/// Whether an item of the spelling submenu is kept.
fn keep_spelling_item(item: &Item) -> bool {
    item.identifier.as_deref().is_some_and(|id| KEPT_SPELLING_IDENTIFIERS.contains(&id))
}

/// Which items of a menu stay: `keep` decides each real item, and a
/// separator stays only when kept content comes before and after it and no
/// other separator is between (none first or last, no two in a row).
fn decide(items: &[Item], keep: impl Fn(&Item) -> bool) -> Vec<bool> {
    let mut kept = vec![false; items.len()];
    let mut pending_separator = None;
    let mut seen_content = false;
    for (index, item) in items.iter().enumerate() {
        if item.separator {
            if seen_content && pending_separator.is_none() {
                pending_separator = Some(index);
            }
        } else if keep(item) {
            if let Some(separator) = pending_separator.take() {
                kept[separator] = true;
            }
            kept[index] = true;
            seen_content = true;
        }
    }
    kept
}

/// Installs the menu filter on the main web view's class. `log_items` logs
/// every menu item offered and whether it was kept, for the window controls
/// check (008 research.md §18). Where the filter leaves no item, the menu is
/// cancelled once it is up, so no menu opens (the owner's decision, 008 spec.md
/// Clarifications 2026-10-11: AppKit adds AutoFill after the hook, and a
/// menu of AutoFill alone isn't offered).
///
/// Called on the `main` web view from `setup()` by `window_controls::apply`
/// (008 T080), in release and E2E builds only (008 research.md §15), on the main
/// thread, which `with_webview` runs it on. Runs once per process: the class
/// is shared by every wry web view, so a second call changes nothing.
pub(crate) fn apply<R: Runtime>(window: &WebviewWindow<R>, log_items: bool) -> tauri::Result<()> {
    window.with_webview(move |platform| {
        LOG_ITEMS.store(log_items, Ordering::Relaxed);
        if let Err(err) = install(platform.inner()) {
            log::error!("the macOS menu filter isn't in place: {err}");
        }
    })
}

/// Whether the hook logs each item (`apply`'s `log_items`).
static LOG_ITEMS: AtomicBool = AtomicBool::new(false);

/// Adds the methods to the class of `web_view`, a pointer to a `WryWebView`
/// (`PlatformWebview::inner`). Runs once; returns an error when the object
/// isn't a wry web view or AppKit refused a method, in which case no menu
/// filtering is in place.
fn install(web_view: *mut c_void) -> Result<(), String> {
    static ONCE: Once = Once::new();
    let mut result = Ok(());
    ONCE.call_once(|| result = add_methods(web_view));
    result
}

fn add_methods(web_view: *mut c_void) -> Result<(), String> {
    if web_view.is_null() {
        return Err("no web view".into());
    }
    // SAFETY: the caller passes a live Objective-C object.
    let object: &AnyObject = unsafe { &*web_view.cast::<AnyObject>() };
    let class = wry_web_view_class(object.class()).ok_or_else(|| {
        format!("the web view is a {:?}, not a WryWebView", object.class().name())
    })?;
    let superclass = class.superclass().ok_or("WryWebView has no superclass")?;
    // SAFETY: `Imp` is a bare function pointer, and the functions below
    // have the signatures their type encodings say.
    unsafe {
        add_method(
            class,
            sel!(validRequestorForSendType:returnType:),
            std::mem::transmute::<
                unsafe extern "C-unwind" fn(
                    &AnyObject,
                    Sel,
                    Option<&AnyObject>,
                    Option<&AnyObject>,
                ) -> *mut AnyObject,
                Imp,
            >(valid_requestor),
            c"@@:@@",
        )?;
        add_method(
            class,
            sel!(willOpenMenu:withEvent:),
            std::mem::transmute::<
                unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, Option<&AnyObject>),
                Imp,
            >(will_open_menu),
            c"v@:@@",
        )?;
        add_method(
            class,
            sel!(quickLookWithEvent:),
            std::mem::transmute::<unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject), Imp>(
                quick_look_with_event,
            ),
            c"v@:@",
        )?;
    }
    let _ = SUPERCLASS.set(superclass);
    log::info!("menu filter installed on WryWebView");
    Ok(())
}

/// The `WryWebView` class itself, found from the web view's class. objc2 names
/// it `wry::wkwebview::class::wry_web_view::WryWebView<version>`, and wry's
/// key-value observers make the instance's class a hidden `NSKVONotifying_…`
/// subclass of it, so the walk goes up the superclasses.
fn wry_web_view_class(mut class: &'static AnyClass) -> Option<&'static AnyClass> {
    loop {
        let name = class.name().to_string_lossy();
        if !name.starts_with("NSKVONotifying_") && name.contains("::wry_web_view::WryWebView") {
            return Some(class);
        }
        class = class.superclass()?;
    }
}

/// WryWebView's superclass, whose `willOpenMenu:withEvent:` runs first.
static SUPERCLASS: std::sync::OnceLock<&'static AnyClass> = std::sync::OnceLock::new();

/// # Safety
/// `imp` has the signature `types` describes.
unsafe fn add_method(
    class: &AnyClass,
    selector: Sel,
    imp: Imp,
    types: &CStr,
) -> Result<(), String> {
    // SAFETY: `class_addMethod` only adds to the class; it returns NO when the
    // class already has the method itself (which a subclass of WKWebView that
    // overrode it would), and we then leave that class alone.
    let added = unsafe {
        class_addMethod(
            class as *const AnyClass as *mut AnyClass,
            selector,
            imp,
            types.as_ptr() as *const c_char,
        )
    };
    if added.as_bool() {
        Ok(())
    } else {
        Err(format!("{selector:?} is already defined on WryWebView"))
    }
}

/// Services: nobody in this web view takes or gives the selection.
unsafe extern "C-unwind" fn valid_requestor(
    _this: &AnyObject,
    _cmd: Sel,
    _send_type: Option<&AnyObject>,
    _return_type: Option<&AnyObject>,
) -> *mut AnyObject {
    std::ptr::null_mut()
}

/// The three-finger-tap Look Up: nothing.
unsafe extern "C-unwind" fn quick_look_with_event(
    _this: &AnyObject,
    _cmd: Sel,
    _event: &AnyObject,
) {
    log::info!("quickLookWithEvent: ignored");
}

/// `-[WryWebView willOpenMenu:withEvent:]`.
unsafe extern "C-unwind" fn will_open_menu(
    this: &AnyObject,
    _cmd: Sel,
    menu: &AnyObject,
    event: Option<&AnyObject>,
) {
    if let Some(superclass) = SUPERCLASS.get() {
        // SAFETY: the superclass's `willOpenMenu:withEvent:` takes the same
        // two arguments; NSView defines it.
        let () =
            unsafe { msg_send![super(this, *superclass), willOpenMenu: menu, withEvent: event] };
    }
    // SAFETY: NSMenu's public property: no context-menu plug-in items.
    let () = unsafe { msg_send![menu, setAllowsContextMenuPlugIns: false] };
    filter_menu(menu);
    // SAFETY: NSMenu's public accessor.
    let left: isize = unsafe { msg_send![menu, numberOfItems] };
    if left == 0 {
        cancel_when_open(menu);
    }
}

/// Cancels `menu` once it is open: AppKit adds AutoFill after the hook, so a
/// menu the filter emptied would show that alone. `cancelTrackingWithoutAnimation`
/// does nothing before tracking starts, so it is scheduled on the run loop in
/// the tracking modes, where it runs once the menu is up.
fn cancel_when_open(menu: &AnyObject) {
    let modes = ["NSEventTrackingRunLoopMode", "NSRunLoopCommonModes", "kCFRunLoopDefaultMode"]
        .map(NSString::from_str);
    let modes = NSArray::from_retained_slice(&modes);
    let nothing: Option<&AnyObject> = None;
    // SAFETY: NSObject's `performSelector:withObject:afterDelay:inModes:`
    // with a selector NSMenu implements, which takes no argument.
    let () = unsafe {
        msg_send![
            menu,
            performSelector: sel!(cancelTrackingWithoutAnimation),
            withObject: nothing,
            afterDelay: 0.0f64,
            inModes: &*modes
        ]
    };
}

// ---------------------------------------------------------------------------
// The menu, through the Objective-C runtime
// ---------------------------------------------------------------------------

fn string_of(value: Option<Retained<NSString>>) -> Option<String> {
    value.map(|s| s.to_string())
}

/// The items of `menu`, as the filter sees them, with the objects.
fn items_of(menu: &AnyObject) -> Vec<(Retained<AnyObject>, Item)> {
    // SAFETY: standard NSMenu / NSMenuItem accessors on live objects.
    unsafe {
        let count: usize = msg_send![menu, numberOfItems];
        (0..count)
            .filter_map(|index| {
                let object: Option<Retained<AnyObject>> =
                    msg_send![menu, itemAtIndex: index as isize];
                let object = object?;
                let identifier: Option<Retained<NSString>> = msg_send![&*object, identifier];
                let action: Option<Sel> = msg_send![&*object, action];
                let separator: bool = msg_send![&*object, isSeparatorItem];
                let tag: isize = msg_send![&*object, tag];
                let item = Item {
                    tag,
                    identifier: string_of(identifier),
                    action: action.map(|action| action.name().to_string_lossy().into_owned()),
                    separator,
                };
                Some((object, item))
            })
            .collect()
    }
}

fn title_of(item: &AnyObject) -> String {
    // SAFETY: `title` is an NSMenuItem accessor.
    let title: Option<Retained<NSString>> = unsafe { msg_send![item, title] };
    string_of(title).unwrap_or_default()
}

fn submenu_of(item: &AnyObject) -> Option<Retained<AnyObject>> {
    // SAFETY: `submenu` is an NSMenuItem accessor.
    unsafe { msg_send![item, submenu] }
}

/// Removes from `menu` everything but what 008 research.md §13 keeps, logging each
/// item. `level` is 0 for the context menu and 1 for the spelling submenu.
fn filter_menu(menu: &AnyObject) {
    filter_level(menu, 0);
}

fn filter_level(menu: &AnyObject, level: usize) {
    let entries = items_of(menu);
    let items: Vec<Item> = entries.iter().map(|(_, item)| item.clone()).collect();
    let keep = |item: &Item| {
        if level == 0 { keep_top_level(item) } else { keep_spelling_item(item) }
    };
    let verdicts = decide(&items, keep);
    let log_items = LOG_ITEMS.load(Ordering::Relaxed);
    for ((object, item), kept) in entries.iter().zip(&verdicts) {
        let submenu = submenu_of(object);
        if log_items {
            log::info!(
                "menu item (level {level}): title={:?} identifier={:?} action={:?} tag={} \
                 separator={} submenu={} -> {}",
                title_of(object),
                item.identifier,
                item.action,
                item.tag,
                item.separator,
                submenu.is_some(),
                if *kept { "kept" } else { "removed" },
            );
        }
        if let Some(submenu) = &submenu {
            if *kept && item.identifier.as_deref() == Some(SPELLING_MENU) {
                filter_level(submenu, level + 1);
            } else if log_items {
                // What is inside an item that is going, for the record.
                for (sub_object, sub_item) in items_of(submenu) {
                    log::info!(
                        "  inside {:?}: title={:?} identifier={:?} action={:?} separator={}",
                        title_of(object),
                        title_of(&sub_object),
                        sub_item.identifier,
                        sub_item.action,
                        sub_item.separator,
                    );
                }
            }
        }
    }
    // Remove from the end, so the indexes stay valid.
    for ((object, _), kept) in entries.iter().zip(&verdicts).rev() {
        if !kept {
            // SAFETY: `removeItem:` takes an item of this menu.
            let () = unsafe { msg_send![menu, removeItem: &**object] };
        }
    }
}

#[cfg(test)]
mod tests {
    use objc2::rc::Allocated;

    use super::*;

    fn item(identifier: &str) -> Item {
        Item { identifier: Some(identifier.into()), action: None, tag: 0, separator: false }
    }

    fn action(action: &str) -> Item {
        Item { identifier: None, action: Some(action.into()), tag: 0, separator: false }
    }

    fn separator() -> Item {
        Item { identifier: None, action: None, tag: 0, separator: true }
    }

    #[test]
    fn top_level_keeps_editing_spelling_and_emoji_only() {
        for kept in [
            "WKMenuItemIdentifierCut",
            "WKMenuItemIdentifierCopy",
            "WKMenuItemIdentifierPaste",
            "WKMenuItemIdentifierSelectAll",
            "WKMenuItemIdentifierSpellingMenu",
            "WKMenuItemIdentifierSpellingGuess",
            "WKMenuItemIdentifierIgnoreSpelling",
            "WKMenuItemIdentifierLearnSpelling",
        ] {
            assert!(keep_top_level(&item(kept)), "{kept}");
        }
        assert!(keep_top_level(&action("orderFrontCharacterPalette:")));
        // Select All without WebKit's identifier (008 FR-011): by its action.
        assert!(keep_top_level(&action("selectAll:")));
        assert!(keep_top_level(&Item {
            identifier: Some("selectAll:".into()),
            action: Some("selectAll:".into()),
            tag: 0,
            separator: false
        }));
        for removed in [
            "WKMenuItemIdentifierLookUp",
            "WKMenuItemIdentifierTranslate",
            "WKMenuItemIdentifierSearchWeb",
            "WKMenuItemIdentifierShareMenu",
            "WKMenuItemIdentifierSpeechMenu",
            "WKMenuItemIdentifierFontMenu",
            "WKMenuItemIdentifierSubstitutionsMenu",
            "WKMenuItemIdentifierTransformationsMenu",
            "WKMenuItemIdentifierWritingDirectionMenu",
            "WKMenuItemIdentifierInspectElement",
            "WKMenuItemIdentifierReload",
        ] {
            assert!(!keep_top_level(&item(removed)), "{removed}");
        }
        assert!(!keep_top_level(&action("showWritingTools:")));
        // Only Select All's own action matches, not a look-alike.
        assert!(!keep_top_level(&action("selectAllChildren:")));
        assert!(!keep_top_level(&action("selectParagraph:")));
        assert!(!keep_top_level(&Item {
            identifier: None,
            action: None,
            tag: 0,
            separator: false
        }));
    }

    #[test]
    fn items_without_a_webkit_identifier_are_kept_by_tag() {
        let tagged = |tag| Item {
            identifier: Some(TAGGED_ACTION.into()),
            action: Some(TAGGED_ACTION.into()),
            tag,
            separator: false,
        };
        // Cut, spelling guess, Ignore and Learn Spelling, as macOS 26 numbers them.
        for tag in [13, 15, 17, 18] {
            assert!(keep_top_level(&tagged(tag)), "{tag}");
        }
        // Look Up (22), Search With Google (21), Translate (95), a tag of none.
        for tag in [22, 21, 95, 0] {
            assert!(!keep_top_level(&tagged(tag)), "{tag}");
        }
        // The tag counts only with WebKit's action.
        let other = Item {
            identifier: None,
            action: Some("performShare:".into()),
            tag: 13,
            separator: false,
        };
        assert!(!keep_top_level(&other));
    }

    #[test]
    fn spelling_submenu_keeps_its_two_items() {
        assert!(keep_spelling_item(&item("WKMenuItemIdentifierShowSpellingPanel")));
        assert!(keep_spelling_item(&item("WKMenuItemIdentifierCheckSpellingWhileTyping")));
        assert!(!keep_spelling_item(&item("WKMenuItemIdentifierCheckSpelling")));
        assert!(!keep_spelling_item(&item("WKMenuItemIdentifierCheckGrammarWithSpelling")));
        assert!(!keep_spelling_item(&action("forwardContextMenuAction:")));
    }

    #[test]
    fn separators_are_tidied() {
        let cut = item("WKMenuItemIdentifierCut");
        let look_up = item("WKMenuItemIdentifierLookUp");
        let copy = item("WKMenuItemIdentifierCopy");
        let keep = |i: &Item| keep_top_level(i);

        // A leading, a doubled and a trailing separator all go.
        let items = [
            look_up.clone(),
            separator(),
            cut.clone(),
            separator(),
            look_up.clone(),
            separator(),
            copy.clone(),
            separator(),
        ];
        assert_eq!(decide(&items, keep), vec![false, false, true, true, false, false, true, false]);

        // A separator between two kept items stays.
        let items = [cut.clone(), separator(), copy.clone()];
        assert_eq!(decide(&items, keep), vec![true, true, true]);

        // Separators with nothing kept around them all go.
        let items = [separator(), look_up.clone(), separator()];
        assert_eq!(decide(&items, keep), vec![false, false, false]);
    }

    /// A real `NSMenu`, as AppKit hands it to the hook.
    #[test]
    fn filters_a_real_menu() {
        fn make(class: &str) -> Retained<AnyObject> {
            let class = AnyClass::get(&std::ffi::CString::new(class).unwrap()).unwrap();
            // SAFETY: alloc/init of an AppKit class.
            unsafe {
                let object: Allocated<AnyObject> = msg_send![class, alloc];
                msg_send![object, init]
            }
        }
        fn add(
            menu: &AnyObject,
            title: &str,
            identifier: Option<&str>,
            tag: isize,
        ) -> Retained<AnyObject> {
            let class = AnyClass::get(c"NSMenuItem").unwrap();
            let title = NSString::from_str(title);
            let key = NSString::from_str("");
            // SAFETY: NSMenuItem's designated initializer and setters.
            unsafe {
                let item: Allocated<AnyObject> = msg_send![class, alloc];
                let item: Retained<AnyObject> = msg_send![item, initWithTitle: &*title, action: sel!(forwardContextMenuAction:), keyEquivalent: &*key];
                if let Some(identifier) = identifier {
                    let identifier = NSString::from_str(identifier);
                    let () = msg_send![&*item, setIdentifier: &*identifier];
                }
                let () = msg_send![&*item, setTag: tag];
                let () = msg_send![menu, addItem: &*item];
                item
            }
        }
        fn separator(menu: &AnyObject) {
            let class = AnyClass::get(c"NSMenuItem").unwrap();
            // SAFETY: NSMenuItem's class method.
            unsafe {
                let item: Retained<AnyObject> = msg_send![class, separatorItem];
                let () = msg_send![menu, addItem: &*item];
            }
        }
        fn titles(menu: &AnyObject) -> Vec<String> {
            items_of(menu)
                .iter()
                .map(|(object, item)| if item.separator { "-".into() } else { title_of(object) })
                .collect()
        }

        let menu = make("NSMenu");
        add(&menu, "brown", None, 15);
        separator(&menu);
        add(&menu, "Look Up", Some("WKMenuItemIdentifierLookUp"), 22);
        add(&menu, "Translate", Some("WKMenuItemIdentifierTranslate"), 95);
        separator(&menu);
        add(&menu, "Cut", None, 13);
        add(&menu, "Copy", Some("WKMenuItemIdentifierCopy"), 8);
        add(&menu, "Paste", Some("WKMenuItemIdentifierPaste"), 14);
        separator(&menu);
        let spelling =
            add(&menu, "Spelling and Grammar", Some("WKMenuItemIdentifierSpellingMenu"), 0);
        let submenu = make("NSMenu");
        add(
            &submenu,
            "Show Spelling and Grammar",
            Some("WKMenuItemIdentifierShowSpellingPanel"),
            36,
        );
        add(&submenu, "Check Document Now", Some("WKMenuItemIdentifierCheckSpelling"), 37);
        separator(&submenu);
        add(
            &submenu,
            "Check Spelling While Typing",
            Some("WKMenuItemIdentifierCheckSpellingWhileTyping"),
            38,
        );
        add(&submenu, "Correct Spelling Automatically", None, 39);
        // SAFETY: attaches the submenu to its item.
        let () = unsafe { msg_send![&*spelling, setSubmenu: &*submenu] };
        separator(&menu);
        let fonts = add(&menu, "Font", None, 0);
        let font_menu = make("NSMenu");
        add(&font_menu, "Bold", None, 40);
        // SAFETY: as above.
        let () = unsafe { msg_send![&*fonts, setSubmenu: &*font_menu] };
        separator(&menu);
        add(&menu, "Inspect Element", Some("WKMenuItemIdentifierInspectElement"), 99);

        filter_menu(&menu);
        assert_eq!(
            titles(&menu),
            ["brown", "-", "Cut", "Copy", "Paste", "-", "Spelling and Grammar"]
        );
        assert_eq!(
            titles(&submenu),
            ["Show Spelling and Grammar", "-", "Check Spelling While Typing"]
        );
    }
}
