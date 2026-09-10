use crate::prelude::*;
use oxc_syntax::node::NodeId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EscErrorIdent(&'static str);

impl EscErrorIdent {
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl Display for EscErrorIdent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// A declaration in a module's current semantic AST (not a petgraph index).
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EscErrorId {
    pub module_id: EscModuleId,
    pub node: NodeId,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EscErrorType {
    Buildin(EscErrorIdent),
    Node(EscErrorId),
}

impl EscErrorType {
    pub const fn builtin(name: &'static str) -> Self {
        Self::Buildin(EscErrorIdent::new(name))
    }

    pub const UNKNOWN: Self = Self::builtin("unknown");

    pub(crate) fn constructor(name: &str) -> Option<Self> {
        EscGlobals::get(name)?.instance_type.map(Self::builtin)
    }
}
