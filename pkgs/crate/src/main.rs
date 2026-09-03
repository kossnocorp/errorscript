mod prelude;

mod cli;
pub use cli::*;

mod config;
pub use config::*;

mod parser;
pub use parser::*;

mod project;
pub use project::*;

#[tokio::main]
async fn main() {
    EscCli::main().await;
}
