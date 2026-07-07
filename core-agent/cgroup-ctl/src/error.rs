use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CgroupError {
    #[error("cgroup v2 filesystem not mounted at {0}")]
    NotMounted(PathBuf),

    #[error("refusing to act on protected process {pid} ({name})")]
    ProtectedProcess { pid: u32, name: String },

    #[error("unexpected format in {path}: {detail}")]
    UnexpectedFormat { path: PathBuf, detail: String },

    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
