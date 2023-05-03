use std::path::PathBuf;
use thisctx::WithContext;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error, WithContext)]
pub enum Error {
    #[error("Can't get standard directories")]
    CannotGetDirectory,
    #[error("IO failed at '{1}'")]
    Io(#[source] std::io::Error, PathBuf),
    #[error("Invalid yaml file at '{1}'")]
    Yaml(#[source] serde_yaml::Error, PathBuf),
    #[error(transparent)]
    Dbot(#[from] dbot::Error),
}
