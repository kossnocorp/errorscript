use crate::prelude::*;

#[derive(Debug)]
pub enum EscProjectState {
    Resolved,
    Parsed(EscProjectStateParsed),
    Checked(EscProjectStateChecked),
}

#[derive(Debug)]
pub struct EscProjectStateParsed {
    pub parsed_files: HashMap<EscModuleId, EscModule>,
}

#[derive(Debug)]
pub struct EscProjectStateChecked {
    pub checked_files: HashMap<EscModuleId, EscModule>,
    pub call_graph: EscCallGraph,
}
