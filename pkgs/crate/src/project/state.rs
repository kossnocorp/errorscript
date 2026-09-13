use crate::prelude::*;

#[derive(Debug)]
pub enum EscProjectState {
    Resolved,
    Parsed(EscProjectStateParsed),
    Checked(Box<EscProjectStateChecked>),
}

#[derive(Debug)]
pub struct EscProjectStateParsed {
    pub parsed_files: HashMap<EscModuleId, EscModule>,
}

#[derive(Debug)]
pub struct EscProjectStateChecked {
    pub checked_files: HashMap<EscModuleId, EscModule>,
    pub call_graph: EscCallGraph,
    /// Escaping body errors (promise rejections for async functions).
    pub errors: HashMap<EscFnId, HashSet<EscErrorType>>,
    pub call_errors: HashMap<(EscModuleId, oxc_syntax::node::NodeId), EscCallErrors>,
}

#[derive(Clone, Debug, Default)]
pub struct EscCallErrors {
    pub sync: HashSet<EscErrorType>,
    pub deferred: HashSet<EscErrorType>,
}
