use std::collections::HashMap;

use uuid::Uuid;

/// Represents a `WebDriver` element reference
#[derive(Debug, Clone)]
pub struct ElementRef {
    /// `WebDriver` element ID (returned to client)
    pub id: String,
    /// JavaScript variable name holding the element reference
    pub js_ref: String,
}

/// Storage for element references within a session
#[derive(Debug, Default)]
pub struct ElementStore {
    elements: HashMap<String, ElementRef>,
}

impl ElementStore {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
        }
    }

    /// Store a new element and return its reference
    pub fn store(&mut self) -> ElementRef {
        let id = Uuid::new_v4().to_string();
        // Remove hyphens from UUID for valid JS variable name
        let id_no_hyphens = id.replace('-', "");
        let js_ref = format!("__wd_el_{id_no_hyphens}");

        let elem_ref = ElementRef {
            id: id.clone(),
            js_ref,
        };

        self.elements.insert(id, elem_ref.clone());
        elem_ref
    }

    /// Get element by `WebDriver` ID
    pub fn get(&self, id: &str) -> Option<&ElementRef> {
        self.elements.get(id)
    }

    /// Adds every web element reference in a script's result to the store.
    /// The execute wrapper keeps each element it returns under `__wd_el_<id>`
    /// (`platform::executor`), as find-element does, but only the store makes
    /// the id usable in later commands (#89). An id that isn't a UUID is left
    /// out, since it becomes part of the scripts that use the element.
    pub fn adopt_references(&mut self, value: &serde_json::Value) {
        match value {
            serde_json::Value::Array(items) => {
                items.iter().for_each(|item| self.adopt_references(item));
            }
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::String(id)) = map.get(ELEMENT_KEY) {
                    if map.len() == 1 && Uuid::parse_str(id).is_ok() {
                        let js_ref = format!("__wd_el_{}", id.replace('-', ""));
                        self.elements.insert(
                            id.clone(),
                            ElementRef {
                                id: id.clone(),
                                js_ref,
                            },
                        );
                    }
                    return;
                }
                map.values().for_each(|item| self.adopt_references(item));
            }
            _ => {}
        }
    }
}

/// The key of a W3C web element reference.
const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_element() {
        let mut store = ElementStore::new();
        let elem = store.store();

        assert!(!elem.id.is_empty());
        assert!(elem.js_ref.starts_with("__wd_el_"));
        // js_ref uses ID without hyphens
        assert!(elem.js_ref.contains(&elem.id.replace('-', "")));
    }

    #[test]
    fn test_get_element() {
        let mut store = ElementStore::new();
        let elem = store.store();
        let id = elem.id.clone();

        let retrieved = store.get(&id).expect("element should exist");
        assert_eq!(retrieved.id, id);
    }

    #[test]
    fn test_js_ref_uses_id_without_hyphens() {
        let mut store = ElementStore::new();
        let elem1 = store.store();
        let elem2 = store.store();

        // js_ref should use ID with hyphens removed for valid JS variable name
        assert_eq!(
            elem1.js_ref,
            format!("__wd_el_{}", elem1.id.replace('-', ""))
        );
        assert_eq!(
            elem2.js_ref,
            format!("__wd_el_{}", elem2.id.replace('-', ""))
        );
    }

    #[test]
    fn adopts_element_references_from_a_script_result() {
        let mut store = ElementStore::new();
        let id = Uuid::new_v4().to_string();
        let nested = Uuid::new_v4().to_string();
        store.adopt_references(&serde_json::json!({
            "found": [{ ELEMENT_KEY: id }],
            "deeper": { "el": { ELEMENT_KEY: nested } },
        }));

        let adopted = store.get(&id).expect("returned element should be stored");
        assert_eq!(adopted.js_ref, format!("__wd_el_{}", id.replace('-', "")));
        assert!(store.get(&nested).is_some());
    }

    #[test]
    fn ignores_ids_that_are_not_uuids() {
        let mut store = ElementStore::new();
        let bad = "x; alert(1)";
        store.adopt_references(&serde_json::json!([{ ELEMENT_KEY: bad }]));
        assert!(store.get(bad).is_none());
    }
}
