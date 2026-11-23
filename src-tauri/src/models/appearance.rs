//! How the page looks on this computer: its color mode and the collection's
//! layout. It lives in `machine.json` and never travels with a database. The
//! main web view keeps no storage of its own (it is incognito, code scanning
//! alert `plaintext-lifetime/cwe-922`), so the page is handed this at start
//! (`main.rs`) and saves it with `set_appearance`.

use serde::{Deserialize, Deserializer, Serialize};

/// Serialized as `"auto"` | `"light"` | `"dark"`, as the frontend's
/// `ThemePreference` has it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the OS's light or dark setting.
    #[default]
    Auto,
    Light,
    Dark,
}

/// Serialized as `"list"` | `"tile"`, as the frontend's `BrowseState["view"]`
/// has it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowseView {
    #[default]
    List,
    Tile,
}

/// Each field is read on its own, with its default for a value this version
/// doesn't know (009 research.md §15), so a later version's value for one
/// doesn't cost the other.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    #[serde(default, deserialize_with = "lenient")]
    pub theme: Theme,
    #[serde(default, deserialize_with = "lenient")]
    pub browse_view: BrowseView,
}

fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_this_version_doesnt_know_reads_as_the_default_and_keeps_the_other() {
        let read: Appearance =
            serde_json::from_str(r#"{"theme":"sepia","browseView":"tile"}"#).unwrap();
        assert_eq!(read, Appearance { theme: Theme::Auto, browse_view: BrowseView::Tile });
        let read: Appearance = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(read, Appearance { theme: Theme::Dark, browse_view: BrowseView::List });
    }
}
