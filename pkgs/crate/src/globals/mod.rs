//! Predefined effects for standard global objects and functions. Entries are
//! contracts for the standard implementations, not guarantees about monkey-patched
//! globals or host I/O failures. Unknown entries are never assumed non-throwing.

use crate::prelude::*;
use std::sync::LazyLock;

mod data;

#[derive(Debug)]
pub struct EscGlobal {
    pub path: &'static str,
    pub value_type: &'static str,
    pub read_errors: &'static [EscErrorType],
    pub call: Option<EscGlobalCall>,
    pub construct: Option<EscGlobalCall>,
    pub instance_type: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
pub struct EscGlobalCall {
    pub errors: &'static [EscErrorType],
    pub returns: &'static str,
    /// Additional effects when argument types cannot rule out coercion,
    /// property access, iteration, or custom formatting.
    pub arguments: &'static [EscGlobalArgument],
    pub min_arguments: usize,
    pub missing_arguments_errors: &'static [EscErrorType],
}

#[derive(Debug)]
pub struct EscGlobalArgument {
    /// None applies the rule to all supplied arguments.
    pub index: Option<usize>,
    pub safe_types: &'static [&'static str],
    pub errors: &'static [EscErrorType],
}

pub struct EscGlobals;

type Properties = HashMap<&'static str, HashMap<&'static str, &'static EscGlobal>>;

impl EscGlobals {
    pub(crate) fn has_path_or_children(path: &str) -> bool {
        Self::get(path).is_some()
            || Self::index().keys().any(|known| {
                known
                    .strip_prefix(path)
                    .is_some_and(|suffix| suffix.starts_with('.'))
            })
    }
    pub(crate) fn is_modified(path: &str, modified: &HashSet<String>) -> bool {
        if modified.contains("globalThis") {
            return true;
        }
        let mut current = path.strip_prefix("globalThis.").unwrap_or(path);
        loop {
            if modified.contains(current) {
                return true;
            }
            let Some((parent, _)) = current.rsplit_once('.') else {
                return false;
            };
            current = parent;
        }
    }
    fn index() -> &'static HashMap<&'static str, &'static EscGlobal> {
        static INDEX: LazyLock<HashMap<&'static str, &'static EscGlobal>> = LazyLock::new(|| {
            data::GLOBALS
                .iter()
                .map(|global| (global.path, global))
                .collect()
        });
        &INDEX
    }

    pub fn get(path: &str) -> Option<&'static EscGlobal> {
        Self::index()
            .get(path.strip_prefix("globalThis.").unwrap_or(path))
            .copied()
    }

    pub fn property(object: &str, property: &str) -> Option<&'static EscGlobal> {
        if object == "globalThis" {
            return Self::index().get(property).copied();
        }
        // Preserve path lookup behavior for unusual compound property names.
        if property.contains('.') {
            return Self::get(&format!("{object}.{property}"));
        }
        static INDEX: LazyLock<Properties> = LazyLock::new(|| {
            let mut index = Properties::new();
            for global in data::GLOBALS {
                if let Some((object, property)) = global.path.rsplit_once('.') {
                    index.entry(object).or_default().insert(property, global);
                }
            }
            index
        });
        INDEX
            .get(object.strip_prefix("globalThis.").unwrap_or(object))?
            .get(property)
            .copied()
    }

    pub fn prototype_property(instance: &str, property: &str) -> Option<&'static EscGlobal> {
        if property.contains('.') {
            return Self::get(&format!("{instance}.prototype.{property}"));
        }
        static INDEX: LazyLock<Properties> = LazyLock::new(|| {
            let mut index = Properties::new();
            for global in data::GLOBALS {
                if let Some((object, property)) = global.path.rsplit_once('.')
                    && let Some(instance) = object.strip_suffix(".prototype")
                {
                    index.entry(instance).or_default().insert(property, global);
                }
            }
            index
        });
        INDEX
            .get(instance.strip_prefix("globalThis.").unwrap_or(instance))?
            .get(property)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_indexes_match_path_lookup_including_global_this() {
        let path = |entry: Option<&'static EscGlobal>| entry.map(|entry| entry.path);
        for global in data::GLOBALS {
            if let Some((object, property)) = global.path.rsplit_once('.') {
                for object in [object.to_owned(), format!("globalThis.{object}")] {
                    assert_eq!(
                        path(EscGlobals::property(&object, property)),
                        path(EscGlobals::get(&format!("{object}.{property}")))
                    );
                }
                if let Some(instance) = object.strip_suffix(".prototype") {
                    assert_eq!(
                        path(EscGlobals::prototype_property(instance, property)),
                        Some(global.path)
                    );
                }
            }
        }
        for object in [
            "globalThis",
            "globalThis.globalThis",
            "JSON",
            "DataView",
            "unknown",
            "missing",
        ] {
            for property in [
                "JSON",
                "globalThis.JSON",
                "parse",
                "prototype.getUint32",
                "missing",
            ] {
                assert_eq!(
                    path(EscGlobals::property(object, property)),
                    path(EscGlobals::get(&format!("{object}.{property}"))),
                    "{object}.{property}"
                );
                assert_eq!(
                    path(EscGlobals::prototype_property(object, property)),
                    path(EscGlobals::get(&format!("{object}.prototype.{property}")))
                );
            }
        }
    }
}
