mod prelude;
use prelude::*;

mod args;
use args::*;

mod cmd;
use cmd::*;

#[derive(Cli)]
#[usage(
    run_async,
    bin = "esc",
    about = "Typed errors in TypeScript without verbosity"
)]
pub struct EscCli {
    #[usage(subcommand)]
    pub command: EscCliCmd,
}

impl EscCli {
    pub async fn main() {
        EscCli::parse().run_async().await.unwrap_or_else(|err| {
            println!("Error: {:?}", err);
            std::process::exit(1);
        });
    }
}
