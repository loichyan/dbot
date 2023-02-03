use clap::Args;
use dbot::Merge;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const V_PATH: &str = "PATH";

#[derive(Debug, Default, Deserialize, Args, Serialize)]
pub struct Options {
    #[arg(long, value_name(V_PATH))]
    pub source: Option<PathBuf>,
    #[arg(long, value_name(V_PATH))]
    pub target: Option<PathBuf>,
}

impl Merge for Options {
    fn merge(&mut self, other: Self) {
        self.source.merge(other.source);
        self.target.merge(other.target);
    }
}

impl Options {
    /// # Panic
    ///
    /// Panics when `source` is `None`.
    pub fn source(&self) -> &Path {
        self.source.as_deref().unwrap()
    }

    /// # Panic
    ///
    /// Panics when `target` is `None`.
    pub fn target(&self) -> &Path {
        self.target.as_deref().unwrap()
    }
}
