//! specs/006-accessory-links contracts/tauri-commands.md "Accessories (new)":
//! the accessory commands. Each `#[tauri::command]` is thin and calls the
//! matching function in `ops`, which takes a `&Connection`.

/// Pure, `Connection`-based business logic, mirroring
/// `commands::firearms::ops`.
pub mod ops {}
