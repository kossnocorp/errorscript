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

        if let EscProjectState::Parsed(state) = &project.state {
            let mut processed_modules = state
                .parsed_files
                .keys()
                .map(EscModuleId::as_str)
                .collect::<Vec<_>>();
            processed_modules.sort_unstable();
            println!("Processed modules: {processed_modules:#?}");
        }

        project.check_files().await?;

        if let EscProjectState::Checked(state) = &project.state {
            let resolved_sccs = state
                .call_graph
                .sccs
                .iter()
                .map(|members| {
                    members
                        .iter()
                        .map(|id| &state.call_graph.graph[id.node()])
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            println!("Resolved function SCCs: {resolved_sccs:#?}");
        }

        Ok(())
    }
}
