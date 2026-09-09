use crate::prelude::*;

use petgraph::graph::NodeIndex;

/// A function's graph identity within a project's current analysis.
/// Graph indices must be rebuilt when the corresponding graph is rebuilt.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EscFnId {
    module_id: EscModuleId,
    node: NodeIndex,
}
