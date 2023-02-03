use std::path::PathBuf;
use thisctx::WithContext;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;
type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Error, WithContext)]
#[thisctx(attr(derive(Debug)), suffix(false), visibility(pub(crate)))]
pub enum Error {
    #[error("A linked or template file cannot have children at '{0}'")]
    UnexpectedChildren(PathBuf),
    #[error("Invalid pattern set found at '{0}'")]
    InvalidPatternSet(PathBuf),
    #[error("Invalid profile at '{0}'")]
    InvalidProfile(PathBuf),
    #[error("IO failed at '{1}'")]
    IoFailed(#[source] std::io::Error, PathBuf),
    #[error("A template cannot be created from a directory: '{0}'")]
    UnexpectedDirectoryForTemplate(PathBuf),
    #[error("Render failed at '{1}'")]
    RenderError(#[source] BoxError, PathBuf),
    #[error("Symlinks are only supported on unix systems: '{0}'")]
    UnsupportedSymlinks(PathBuf),
}
