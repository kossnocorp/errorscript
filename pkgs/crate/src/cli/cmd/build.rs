use crate::cli::prelude::*;

#[derive(Args, Debug)]
pub struct EscCliCmdBuild {
    #[usage(flatten)]
    project_args: EscCliArgsProject,
}

impl RunAsync for EscCliCmdBuild {
    type Output = Result<()>;

    async fn run_async(self) -> Self::Output {
        let config = EscConfig::resolve(self.project_args.project.as_ref())?;
        println!("Config: {:?}", config);
        Ok(())
    }
}
