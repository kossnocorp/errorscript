mod prelude {
    pub use crate::*;
    pub use anyhow::Result;
    pub use std::path::PathBuf;
}

pub use errorscript::*;

mod cli;
pub use cli::*;

#[tokio::main]
async fn main() {
    EscCli::main().await;
}
