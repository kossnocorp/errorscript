use crate::prelude::*;

type EscProjectParseQueue = Arc<tokio::sync::Mutex<Vec<EscModulePath>>>;

impl EscProject {
    pub async fn parse_files(&mut self) -> Result<()> {
        let files: EscProjectParseQueue = Arc::new(tokio::sync::Mutex::new(
            self.files().await?.into_iter().collect(),
        ));
        let mut parsed_files = HashMap::new();
        let mut scheduled = HashSet::new();
        let mut tasks = tokio::task::JoinSet::new();
        let resolver = self.resolver.clone();

        loop {
            let pending = {
                let mut files = files.lock().await;
                files
                    .drain(..)
                    .filter(|path| scheduled.insert(path.clone()))
                    .collect::<Vec<_>>()
            };

            for path in pending {
                let path = path.clone();
                let files = Arc::clone(&files);
                let resolver = resolver.clone();
                tasks.spawn(Self::parse_file(path, files, resolver));
            }

            let Some(file) = tasks.join_next().await else {
                break;
            };
            let (path, file) = file
                .context("File loading task failed")?
                .context("Failed to load project file")?;
            parsed_files.insert(self.module_id(&path)?, file);
        }

        self.state = EscProjectState::Parsed(EscProjectStateParsed { parsed_files });

        Ok(())
    }

    async fn parse_file(
        path: EscModulePath,
        files: EscProjectParseQueue,
        resolver: EscResolver,
    ) -> Result<(EscModulePath, EscModule)> {
        let source_code = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read project file at {path}"))?;

        let file = EscModule::parse(source_code, &path, &resolver)?;

        let dependencies = file.extract_dependencies();
        files.lock().await.extend(dependencies);

        Ok((path, file))
    }
}
