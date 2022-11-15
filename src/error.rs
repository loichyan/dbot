use std::path::PathBuf;
use thisctx::WithContext;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error, WithContext)]
#[thisctx(visibility(pub(crate)))]
#[thisctx(attr(derive(Debug)))]
pub enum Error {
    #[error("A linked or template file cannot have children: '{0}'")]
    UnexpectedChildren(PathBuf),
    #[error("Invalid pattern set found: '{0}'")]
    InvalidPatternSet(PathBuf),
    #[error("Invalid profile: '{0}'")]
    InvalidProfile(PathBuf),
    #[error("{source}: '{path}'")]
    IoFailed {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("A template cannot be created from a directory: '{0}'")]
    UnexpectedDirectoryForTemplate(PathBuf),
}
