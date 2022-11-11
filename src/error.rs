use std::path::PathBuf;

use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("a linked or template file cannot have children: '{}'", .0.display())]
    UnexpectedChildren(PathBuf),
    #[error("invalid pattern set found: '{}'", .0.display())]
    InvalidPatternSet(PathBuf),
}
