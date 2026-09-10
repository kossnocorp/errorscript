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
}

#[derive(Debug, Default)]
pub struct EscCallGraph {
    /// Caller -> possible callee. Parallel edges retain distinct call sites.
    pub graph: DiGraph<EscFunction, EscCallSite>,
    pub calls: Vec<EscCall>,
    /// Tarjan's reverse topological order: callee components precede callers.
    /// Members remain separate functions for catch-sensitive error analysis.
    pub sccs: Vec<Vec<EscFnId>>,
    pub component_of: HashMap<EscFnId, usize>,
}
