use crate::prelude::*;

#[derive(Debug)]
pub enum EscProjectState {
    Resolved,
    Parsed(EscProjectStateParsed),
}

#[derive(Debug)]
pub struct EscProjectStateParsed {
    pub parsed_files: HashMap<EscModulePath, EscModule>,
}

#[derive(Debug)]
pub struct EscProjectStateChecked {
    pub checked_files: HashMap<EscModulePath, EscModule>,
}
