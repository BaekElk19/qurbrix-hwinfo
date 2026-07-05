use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobResult {
    pub pattern: String,
    pub paths: Vec<PathBuf>,
}
