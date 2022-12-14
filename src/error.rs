use std::path::PathBuf;
use thisctx::WithContext;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error, WithContext)]
#[thisctx(attr(derive(Debug)))]
#[thisctx(visibility(pub(crate)))]
#[thisctx(suffix(false))]
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
    #[error("{source}: '{path}'")]
    InvalidTemplate { source: tera::Error, path: PathBuf },
    #[error("Symlinks are only supported on unix systems: '{0}'")]
    UnsupportedSymlinks(PathBuf),
}
