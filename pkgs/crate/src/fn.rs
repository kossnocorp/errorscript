use crate::prelude::*;

use oxc_syntax::node::NodeId;
use petgraph::graph::{DiGraph, NodeIndex};

/// A function's graph identity within a project's current analysis.
/// Graph indices must be rebuilt when the corresponding graph is rebuilt.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EscFnId {
    module_id: EscModuleId,
    node: NodeIndex,
}

impl EscFnId {
    pub(crate) fn new(module_id: EscModuleId, node: NodeIndex) -> Self {
        Self { module_id, node }
    }

    pub fn module_id(&self) -> &EscModuleId {
        &self.module_id
    }

    pub fn node(&self) -> NodeIndex {
        self.node
    }
}

/// Owned address of a function's syntax in the corresponding module's Semantic.
#[derive(Debug)]
pub struct EscFunction {
    pub module_id: EscModuleId,
    pub node_id: NodeId,
    pub name: Option<String>,
    pub is_async: bool,
    pub is_generator: bool,
}

#[derive(Clone, Debug)]
pub struct EscCallSite {
    pub module_id: EscModuleId,
    pub node_id: NodeId,
}

#[derive(Debug)]
pub struct EscCall {
    /// None for module-level execution.
    pub caller: Option<EscFnId>,
    pub site: EscCallSite,
    pub targets: Vec<EscFnId>,
    /// Additional targets may exist. This call must not be treated as error-free.
    pub unresolved: bool,
    /// Known built-in/class instance types produced by a constructor call.
    pub constructed_types: HashSet<EscErrorType>,
    /// Canonical paths into the predefined global-effects table.
    pub global_calls: HashSet<&'static str>,
}

#[derive(Debug, Default)]
pub struct EscCallGraph {
    /// Declaration signatures linked through paired runtime/type module exports.
    pub(crate) signatures: HashMap<EscFnId, Vec<EscErrorId>>,
    /// Caller -> possible callee. Parallel edges retain distinct call sites.
    pub graph: DiGraph<EscFunction, EscCallSite>,
    pub calls: Vec<EscCall>,
    /// Tarjan's reverse topological order: callee components precede callers.
    /// Members remain separate functions for catch-sensitive error analysis.
    pub sccs: Vec<Vec<EscFnId>>,
    pub component_of: HashMap<EscFnId, usize>,
    pub(crate) type_references: HashMap<EscErrorId, HashSet<EscErrorType>>,
    pub(crate) super_types: HashMap<EscErrorType, HashSet<EscErrorType>>,
    pub(crate) modified_globals: HashSet<String>,
}

impl EscCallGraph {
    pub(crate) fn global(&self, path: &str) -> Option<&'static EscGlobal> {
        let global = EscGlobals::get(path)?;
        if self.modified_globals.contains("globalThis")
            || self
                .modified_globals
                .contains(global.path.split('.').next()?)
        {
            return None;
        }
        Some(global)
    }
}
