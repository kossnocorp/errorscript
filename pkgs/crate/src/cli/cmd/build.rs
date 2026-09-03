use crate::cli::prelude::*;

#[derive(Args, Debug)]
pub struct EscCliCmdBuild {
    #[usage(flatten)]
    project_args: EscCliArgsProject,
}

impl RunAsync for EscCliCmdBuild {
    type Output = Result<()>;

    async fn run_async(self) -> Self::Output {
        println!("Hello {:?}", self);
        Ok(())
    }
}
