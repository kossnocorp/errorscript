mod prelude;

mod path;
pub use path::*;

mod cli;
pub use cli::*;

mod config;
pub use config::*;

mod module;
pub use module::*;

mod r#fn;
pub use r#fn::*;

mod resolver;
pub use resolver::*;

mod project;
pub use project::*;

mod repo;
pub use repo::*;

mod checker;
pub use checker::*;

#[tokio::main]
async fn main() {
    EscCli::main().await;
}
