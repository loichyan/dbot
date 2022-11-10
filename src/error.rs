use std::path::PathBuf;

use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("a linked or template file cannot have children: '{}'", .0.display())]
    UnexpectedChildren(PathBuf),
    #[error("")]
    InvalidPatternSet(PathBuf),
}
