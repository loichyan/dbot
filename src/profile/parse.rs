use super::{path_only_attr, AttrType, ProfileAttrBuilder};
use std::path::{Component, Path, PathBuf};
use thisctx::IntoError;
use tracing::warn;

pub(super) fn parse_attribute(s: &str) -> ParseResult<ProfileAttrBuilder> {
    Parser {
        index: 0,
        start: 0,
        source: s,
    }
    .parse()
}

struct Parser<'a> {
    index: usize,
    start: usize,
    source: &'a str,
}

type ParseResult<'a, T> = Result<T, ParseError<'a>>;

enum Value<'a> {
    String(&'a str),
    True,
    False,
}

mod error {
    use thisctx::WithContext;
    use thiserror::Error;

    #[derive(Debug, Error, WithContext)]
    #[thisctx(suffix(false))]
    pub(crate) enum ParseError<'a> {
        #[error("a path can't start with a prefix")]
        PathStartsWithPrefix,
        #[error("unexpected end of string")]
        UnexpectedEos,
        #[error("unexpected char '{0}'")]
        UnexpectedCharater(#[thisctx(generic(false))] char),
        #[error("unterminated string")]
        UnterminatedString,
        #[error("unexpected identifier '{0}'")]
        UnexpectedIdentifier(&'a str),
        #[error("value of '{0}' cannot be a {1}")]
        UnexpectedValueType(&'static str, &'static str),
        #[error("duplicate '{0}' attribute")]
        DuplicateAttribute(&'static str),
        #[error("unterminated angle bracket")]
        UnterminatedAngleBracket,
        #[error("invalid type '{0}'")]
        InvalidType(&'a str),
    }
}
pub(crate) use error::ParseError;

macro_rules! ident_start {
    () => {
        b'a'..=b'z' | b'A'..=b'Z' | b'_'
    };
}

impl<'a> Parser<'a> {
    fn buf_start(&mut self) {
        self.start = self.index;
    }

    fn buf_end(&self) -> &'a str {
        self.buf_to(self.index)
    }

    fn buf_to(&self, end: usize) -> &'a str {
        self.source
            .get(self.start..end)
            .unwrap_or_else(|| unreachable!())
    }

    fn byte(&self) -> Option<u8> {
        self.byte_at(self.index)
    }

    fn byte_at(&self, i: usize) -> Option<u8> {
        self.source.as_bytes().get(i).copied()
    }

    fn last_char(&self) -> char {
        self.char_at(self.index - 1)
            .unwrap_or_else(|| unreachable!())
    }

    fn char_at(&self, i: usize) -> Option<char> {
        self.source
            .get(i..)
            .unwrap_or_else(|| unreachable!())
            .chars()
            .next()
    }

    fn next(&mut self) -> Option<u8> {
        if let Some(ch) = self.byte() {
            self.advance();
            Some(ch)
        } else {
            None
        }
    }

    fn advance(&mut self) {
        self.index += 1;
    }

    fn parse(&mut self) -> ParseResult<'a, ProfileAttrBuilder> {
        self.skip_spaces();
        if !matches!(self.next(), Some(b'<')) {
            Ok(path_only_attr(normalize_path(self.source)?))
        } else {
            self.parse_xml_like()
        }
    }

    fn parse_xml_like(&mut self) -> ParseResult<'a, ProfileAttrBuilder> {
        let mut attr = ProfileAttrBuilder::default();

        self.skip_spaces();
        self.buf_start();
        self.expect(|ch| matches!(ch, ident_start!()))?;
        match self.next_ident() {
            "copy" => attr.ty = Some(AttrType::Copy),
            "link" => attr.ty = Some(AttrType::Link),
            "template" => attr.ty = Some(AttrType::Template),
            ty => return error::InvalidType(ty).fail(),
        }

        loop {
            self.skip_spaces();
            self.buf_start();
            match self.next() {
                Some(ch) => match ch {
                    ident_start!() => self.next_attribute(&mut attr)?,
                    b'>' => break,
                    _ => return error::UnexpectedCharater(self.last_char()).fail(),
                },
                None => return error::UnterminatedAngleBracket.fail(),
            }
        }
        Ok(attr)
    }

    fn next_attribute(&mut self, attr: &mut ProfileAttrBuilder) -> ParseResult<'a, ()> {
        macro_rules! check_dup {
            ($name:ident) => {
                if attr.$name.is_some() {
                    return error::DuplicateAttribute(stringify!($name)).fail();
                }
            };
        }

        let key = self.next_ident();
        self.skip_spaces();
        let val = if matches!(self.byte(), Some(b'=')) {
            self.advance();
            self.skip_spaces();
            self.next_value()?
        } else {
            Value::True
        };
        match key {
            "recursive" => {
                check_dup!(recursive);
                match val {
                    Value::True => attr.recursive = Some(true),
                    Value::False => attr.recursive = Some(false),
                    _ => return error::UnexpectedValueType("recursive", "bool").fail(),
                }
            }
            "source" => {
                check_dup!(source);
                if let Value::String(s) = val {
                    attr.source = Some(normalize_path(s)?)
                } else {
                    return error::UnexpectedValueType("source", "string").fail();
                }
            }
            _ => warn!("Undefined attribute '{}'", key),
        }

        Ok(())
    }

    fn next_ident(&mut self) -> &'a str {
        while matches!(self.byte(), Some(ident_start!() | b'0'..=b'9')) {
            self.advance();
        }
        self.buf_end()
    }

    fn next_value(&mut self) -> ParseResult<'a, Value<'a>> {
        self.buf_start();
        match self.next() {
            Some(ch) => match ch {
                b'"' => self.next_string(b'"').map(Value::String),
                b'\'' => self.next_string(b'\'').map(Value::String),
                ident_start!() => match self.next_ident() {
                    "true" => Ok(Value::True),
                    "false" => Ok(Value::False),
                    ident => error::UnexpectedIdentifier(ident).fail(),
                },
                _ => error::UnexpectedCharater(self.last_char()).fail(),
            },
            None => error::UnexpectedEos.fail(),
        }
    }

    fn next_string(&mut self, quote: u8) -> ParseResult<'a, &'a str> {
        self.buf_start();
        while let Some(ch) = self.next() {
            if ch == quote {
                return Ok(self.buf_to(self.index - 1));
            }
        }
        error::UnexpectedEos.fail()
    }

    fn skip_spaces(&mut self) {
        while matches!(self.byte(), Some(b' ')) {
            self.advance();
        }
    }

    fn expect(&mut self, check: impl FnOnce(u8) -> bool) -> ParseResult<'a, ()> {
        match self.next() {
            Some(ch) => {
                if check(ch) {
                    Ok(())
                } else {
                    error::UnexpectedCharater(self.last_char()).fail()
                }
            }
            None => error::UnexpectedEos.fail(),
        }
    }
}

pub(super) fn normalize_path(path: &str) -> ParseResult<PathBuf> {
    let mut buf = PathBuf::new();
    for compo in Path::new(path).components() {
        match compo {
            Component::CurDir | Component::RootDir => (),
            Component::ParentDir => {
                buf.pop();
            }
            Component::Prefix(_) => return error::PathStartsWithPrefix.fail(),
            _ => buf.push(compo),
        }
    }
    Ok(buf)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn create_normalized_path() {
        assert_eq!(
            PathBuf::from("abc/def"),
            normalize_path("skip/../abc/def").unwrap()
        );
        assert_eq!(
            PathBuf::from("abc/def"),
            normalize_path("/abc/def").unwrap()
        );
        assert_eq!(
            PathBuf::from("abc/def"),
            normalize_path("abc/./def").unwrap()
        );
    }

    fn type_only_attr(ty: AttrType) -> ProfileAttrBuilder {
        ProfileAttrBuilder {
            ty: Some(ty),
            ..Default::default()
        }
    }

    #[test]
    fn parse_non_xml_like() {
        assert_eq!(
            parse_attribute("path/to/source").unwrap(),
            path_only_attr("path/to/source")
        );
    }

    #[test]
    fn parse_type_only() {
        assert_eq!(
            parse_attribute("<template>").unwrap(),
            type_only_attr(AttrType::Template)
        );
        assert_eq!(
            parse_attribute("<link>").unwrap(),
            type_only_attr(AttrType::Link)
        );
        assert_eq!(
            parse_attribute("<copy>").unwrap(),
            type_only_attr(AttrType::Copy)
        );
    }

    #[test]
    fn parse_recursive() {
        fn expect_link_and_recursive(recursive: bool) -> ProfileAttrBuilder {
            ProfileAttrBuilder {
                ty: Some(AttrType::Link),
                recursive: Some(recursive),
                ..Default::default()
            }
        }
        assert_eq!(
            parse_attribute("<link recursive>").unwrap(),
            expect_link_and_recursive(true)
        );
        assert_eq!(
            parse_attribute(r#"<link recursive=true>"#).unwrap(),
            expect_link_and_recursive(true)
        );
        assert_eq!(
            parse_attribute(r#"<link recursive=false>"#).unwrap(),
            expect_link_and_recursive(false)
        );
    }

    #[test]
    fn parse_source() {
        fn expect_link_and_source(source: &str) -> ProfileAttrBuilder {
            ProfileAttrBuilder {
                ty: Some(AttrType::Link),
                source: Some(source.into()),
                ..Default::default()
            }
        }

        assert_eq!(
            parse_attribute(r#"<link source="path/to/source">"#).unwrap(),
            expect_link_and_source("path/to/source")
        );
        assert_eq!(
            parse_attribute(r#"<link source='path/to/source'>"#).unwrap(),
            expect_link_and_source("path/to/source")
        );
    }

    #[test]
    fn skip_spaces() {
        let expected = ProfileAttrBuilder {
            ty: Some(AttrType::Template),
            source: Some("path/to/source".into()),
            recursive: Some(false),
            ..Default::default()
        };
        let s = r#"  <  template  recursive  =  false  source  =  "path/to/source"  >  "#;
        assert_eq!(parse_attribute(s).unwrap(), expected);
    }
}
