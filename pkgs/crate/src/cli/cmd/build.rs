use crate::cli::prelude::*;

#[derive(Args, Debug)]
pub struct EscCliCmdBuild {
    #[usage(flatten)]
    project_args: EscCliArgsProject,
}

impl RunAsync for EscCliCmdBuild {
    type Output = Result<()>;

    async fn run_async(self) -> Self::Output {
        let mut project = EscProject::resolve(self.project_args.project.as_ref()).await?;
        project.parse_files().await?;
        // let files = project.files().await?;
        // println!("Files: {:?}", files);
        Ok(())
    }
}
