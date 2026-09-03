use crate::cli::prelude::*;

#[derive(Args, Debug)]
pub struct EscCliArgsProject {
    /// Use the given project or `tsconfig.json` path instead of the cwd.
    #[usage(short, long, value_name = "PROJECT_PATH")]
    pub project_path: Option<PathBuf>,
}
