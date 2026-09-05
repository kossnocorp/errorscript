use crate::prelude::*;

#[derive(Debug)]
pub enum EscProjectState {
    Resolved,
    Parsed(EscProjectStateParsed),
}

#[derive(Debug)]
pub struct EscProjectStateParsed {
    pub parsed_files: HashMap<PathBuf, EscParserFile>,
}
