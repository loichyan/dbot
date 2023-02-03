mod de;

use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PatternSetBuilder {
    globs: Vec<Glob>,
}

impl PatternSetBuilder {
    pub fn iter(&self) -> impl Iterator<Item = &Glob> {
        self.globs.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = Glob> {
        self.globs.into_iter()
    }

    pub fn is_empty(&self) -> bool {
        self.globs.is_empty()
    }

    pub fn build(&self) -> Result<PatternSet, globset::Error> {
        let mut builder = GlobSetBuilder::new();
        for pat in self.globs.iter().cloned() {
            builder.add(pat);
        }
        builder.build().map(PatternSet)
    }
}

impl Extend<Glob> for PatternSetBuilder {
    fn extend<T: IntoIterator<Item = Glob>>(&mut self, iter: T) {
        self.globs.extend(iter);
    }
}

#[derive(Clone, Debug, Default)]
pub struct PatternSet(GlobSet);

#[cfg(test)]
impl PartialEq for PatternSet {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[cfg(test)]
impl Eq for PatternSet {}

impl PatternSet {
    pub fn is_match<P: AsRef<Path>>(&self, path: P) -> bool {
        self.0.is_match(path.as_ref())
    }
}
