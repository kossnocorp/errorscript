use super::prelude::*;

mod build;
use build::*;

#[derive(Subcommands)]
#[usage(run_async)]
pub enum EscCliCmd {
    /// Builds project
    Build(EscCliCmdBuild),
}
