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

impl EscGlobals {
    pub fn get(path: &str) -> Option<&'static EscGlobal> {
        static INDEX: LazyLock<HashMap<&'static str, &'static EscGlobal>> = LazyLock::new(|| {
            data::GLOBALS
                .iter()
                .map(|global| (global.path, global))
                .collect()
        });
        INDEX
            .get(path.strip_prefix("globalThis.").unwrap_or(path))
            .copied()
    }

    pub fn property(object: &str, property: &str) -> Option<&'static EscGlobal> {
        Self::get(&format!("{object}.{property}"))
    }
}
