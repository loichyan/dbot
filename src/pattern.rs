mod de;

use globset::{Glob, GlobSet, GlobSetBuilder};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PatternSetBuilder(Vec<Pattern>);

pub struct PatternSet(GlobSet);

impl PatternSetBuilder {
    pub fn build(self) -> Option<PatternSet> {
        let mut builder = GlobSetBuilder::new();
        for s in self.0 {
            builder.add(s.0);
        }
        builder.build().map(PatternSet).ok()
    }

    pub fn extend(&self, other: &PatternSetBuilder) -> PatternSetBuilder {
        Self(Vec::from_iter(
            self.0.iter().cloned().chain(other.0.iter().cloned()),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern(Glob);
