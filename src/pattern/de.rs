use super::PatternSetBuilder;
use globset::Glob;
use serde::{de::Visitor, Deserialize};
use std::fmt;

struct Pattern(Glob);

struct PatternVisitor;

impl<'de> Visitor<'de> for PatternVisitor {
    type Value = Pattern;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a glob pattern")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Pattern(
            Glob::new(v).map_err(|_| E::custom("invalid pattern format"))?,
        ))
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(PatternVisitor)
    }
}

struct PatternSetVisitor;

impl<'de> Visitor<'de> for PatternSetVisitor {
    type Value = PatternSetBuilder;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a set of glob patterns")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let pat = PatternVisitor.visit_str(v)?;
        Ok(PatternSetBuilder { globs: vec![pat.0] })
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut builder = Vec::with_capacity(seq.size_hint().unwrap_or_default());
        while let Some(pat) = seq.next_element::<Pattern>()? {
            builder.push(pat.0);
        }
        Ok(PatternSetBuilder { globs: builder })
    }
}

impl<'de> Deserialize<'de> for PatternSetBuilder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(PatternSetVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_pattern() {
        serde_yaml::from_str::<Pattern>("abc/**/def").unwrap();
    }

    #[test]
    fn deserialize_pattern_set_builder() {
        serde_yaml::from_str::<PatternSetBuilder>("abc/**/def").unwrap();
        serde_yaml::from_str::<PatternSetBuilder>(
            r#"
            - abc/**/def
            - xyz/**
            "#,
        )
        .unwrap();
    }
}
