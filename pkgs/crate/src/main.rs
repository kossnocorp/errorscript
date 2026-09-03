mod prelude;

mod cli;
pub use cli::*;

mod parser;
pub use parser::*;

#[tokio::main]
async fn main() {
    EscCli::main().await;
}
