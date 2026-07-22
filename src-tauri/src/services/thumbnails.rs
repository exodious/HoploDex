//! Generic-thumbnail fallback resolution (FR-009, research.md §10): a
//! firearm with no photos shows its `FirearmType`'s bundled generic
//! thumbnail instead. The resolution decision itself is a pure function,
//! independently testable; only the actual byte-loading (an app-resource
//! filesystem read, needing a `tauri::AppHandle`) lives in
//! `commands::photos`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThumbnailRef {
    Photo(i64),
    Generic(String),
}

/// `Firearm.thumbnail_photo_id` is the source of truth for which photo is
/// the thumbnail; falling back to the type's generic thumbnail only when
/// none is set (data-model.md's Photo entity notes).
pub fn resolve(thumbnail_photo_id: Option<i64>, generic_thumbnail_key: &str) -> ThumbnailRef {
    match thumbnail_photo_id {
        Some(id) => ThumbnailRef::Photo(id),
        None => ThumbnailRef::Generic(generic_thumbnail_key.to_string()),
    }
}

/// Path (relative to the Tauri resource directory) of a generic
/// thumbnail's bundled PNG asset, matching `tauri.conf.json`'s
/// `bundle.resources` entry and `src-tauri/resources/thumbnails/*.png`.
pub fn generic_thumbnail_resource_path(key: &str) -> String {
    format!("resources/thumbnails/{key}.png")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_photo_thumbnail_takes_precedence_over_the_generic_one() {
        assert_eq!(resolve(Some(42), "handgun"), ThumbnailRef::Photo(42));
    }

    #[test]
    fn no_photo_falls_back_to_the_generic_thumbnail() {
        assert_eq!(resolve(None, "handgun"), ThumbnailRef::Generic("handgun".to_string()));
    }

    #[test]
    fn resource_path_matches_the_bundled_asset_layout() {
        assert_eq!(generic_thumbnail_resource_path("rifle"), "resources/thumbnails/rifle.png");
    }
}
