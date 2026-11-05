use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemifSetupError {
    ExistingPathIsNotRepository(PathBuf),
    DirtyCheckout {
        directory: PathBuf,
        status: String,
    },
    UnexpectedRepositoryOrigin {
        directory: PathBuf,
        origin: String,
    },
    MissingExecutable(PathBuf),
    UnexpectedModelSize {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    Command {
        context: String,
        message: String,
    },
    Io {
        context: String,
        message: String,
    },
}

impl std::fmt::Display for SemifSetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExistingPathIsNotRepository(path) => write!(
                f,
                "{} exists but is not a SemIf git checkout",
                path.display()
            ),
            Self::DirtyCheckout { directory, status } => write!(
                f,
                "{} contains local modifications; refusing to install modified code as pinned SemIf: {}",
                directory.display(),
                status.replace('\n', "; ")
            ),
            Self::UnexpectedRepositoryOrigin { directory, origin } => write!(
                f,
                "{} points to unexpected git origin {origin:?}",
                directory.display()
            ),
            Self::MissingExecutable(path) => {
                write!(f, "SemIf installation did not create {}", path.display())
            }
            Self::UnexpectedModelSize {
                path,
                expected,
                actual,
            } => write!(
                f,
                "{} has {actual} bytes; expected {expected}",
                path.display()
            ),
            Self::Command { context, message } => {
                write!(f, "could not {context}: {message}")
            }
            Self::Io { context, message } => {
                write!(f, "could not {context}: {message}")
            }
        }
    }
}

impl std::error::Error for SemifSetupError {}
