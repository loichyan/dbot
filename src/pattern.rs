mod de;

use globset::{Glob, GlobSet, GlobSetBuilder};

#[derive(Debug, Default, PartialEq, Eq)]
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
}

impl IntoIterator for PatternSetBuilder {
    type IntoIter = <Vec<Pattern> as IntoIterator>::IntoIter;
    type Item = Pattern;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Extend<Pattern> for PatternSetBuilder {
    fn extend<T: IntoIterator<Item = Pattern>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Pattern(Glob);
