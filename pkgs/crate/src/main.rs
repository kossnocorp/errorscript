mod prelude;

mod cli;
pub use cli::*;

mod config;
pub use config::*;

mod parser;
pub use parser::*;

#[tokio::main]
async fn main() {
    EscCli::main().await;
}
