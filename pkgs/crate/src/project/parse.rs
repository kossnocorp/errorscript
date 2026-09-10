use crate::prelude::*;

use std::collections::VecDeque;

impl EscProject {
    pub async fn parse_files(&mut self) -> Result<()> {
        let mut files = self.files().await?.into_iter().collect::<VecDeque<_>>();
        let mut parsed_files = HashMap::new();
        let mut scheduled = HashSet::new();
        let mut tasks = tokio::task::JoinSet::new();
        let resolver = self.resolver.clone();
        let workers = std::thread::available_parallelism().map_or(1, usize::from);

        loop {
            while tasks.len() < workers
                && let Some(path) = files.pop_front()
            {
                if !scheduled.insert(path.clone()) {
                    continue;
                }
                let resolver = resolver.clone();
                tasks.spawn_blocking(move || Self::parse_file(path, resolver));
            }

            let Some(file) = tasks.join_next().await else {
                break;
            };
            let (path, file) = file
                .context("File loading task failed")?
                .context("Failed to load project file")?;
            files.extend(file.extract_dependencies());
            parsed_files.insert(self.module_id(&path)?, file);
        }

        self.state = EscProjectState::Parsed(EscProjectStateParsed { parsed_files });

        Ok(())
    }

    fn parse_file(
        path: EscModulePath,
        resolver: EscResolver,
    ) -> Result<(EscModulePath, EscModule)> {
        let source_code = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read project file at {path}"))?;

        let file = EscModule::parse(source_code, &path, &resolver)?;

        Ok((path, file))
    }
}
