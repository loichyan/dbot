#[macro_use]
mod utils;

/// Apply profiles to target path.
mod apply;
pub use apply::{apply, TemplateRenderer};

/// Compile profiles.
pub mod compile;
pub use compile::compile;

/// Error shared among modules.
pub mod error;
pub use error::Error;

/// Helper trait to merge data structures.
mod merge;
pub use merge::Merge;

/// Patterns used to match paths.
mod pattern;

/// Use defined profiles.
pub mod profile;
pub use profile::Profile;
