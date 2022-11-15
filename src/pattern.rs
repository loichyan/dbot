mod de;

use globset::{Glob, GlobSet, GlobSetBuilder};
use once_cell::sync::OnceCell;
use std::{path::Path, rc::Rc};

#[derive(Clone, Debug, Default)]
pub struct PatternSetBuilder {
    builder: Vec<Glob>,
    set: OnceCell<Rc<PatternSet>>,
}

impl PartialEq for PatternSetBuilder {
    fn eq(&self, other: &Self) -> bool {
        self.builder.eq(&other.builder)
    }
}

impl Eq for PatternSetBuilder {}

impl PatternSetBuilder {
    pub fn extend(&self, other: &PatternSetBuilder) -> PatternSetBuilder {
        Self {
            builder: self
                .builder
                .iter()
                .cloned()
                .chain(other.builder.iter().cloned())
                .collect(),
            set: <_>::default(),
        }
    }

    pub fn build(&self) -> Result<Rc<PatternSet>, globset::Error> {
        self.set
            .get_or_try_init(|| {
                let mut builder = GlobSetBuilder::new();
                for pat in self.builder.iter().cloned() {
                    builder.add(pat);
                }
                builder.build().map(PatternSet).map(Rc::new)
            })
            .cloned()
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
