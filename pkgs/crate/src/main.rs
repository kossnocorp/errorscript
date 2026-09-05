mod prelude;

mod cli;
pub use cli::*;

mod config;
pub use config::*;

mod module;
pub use module::*;

mod resolver;
pub use resolver::*;

mod project;
pub use project::*;

#[tokio::main]
async fn main() {
    EscCli::main().await;
}
