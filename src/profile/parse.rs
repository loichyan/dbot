use super::{path_only_attr, AttrType, ProfileAttrBuilder};
use dotfish::DotFish;
use std::path::{Component, Path, PathBuf};
use tracing::{instrument, warn};

pub(super) fn parse_attribute(s: &str) -> ParseResult<ProfileAttrBuilder> {
    let mut parser = Parser {
        index: 0,
        start: 0,
        source: s,
    };
    parser.parse()
}

struct Parser<'a> {
    index: usize,
    start: usize,
    source: &'a str,
}

type ParseResult<T> = Result<T, String>;

macro_rules! ident_pat {
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
        self.source.get(self.start..end).unwrap_unreachable2()
    }

    fn byte(&self) -> Option<u8> {
        self.byte_at(self.index)
    }

    fn byte_at(&self, i: usize) -> Option<u8> {
        self.source.as_bytes().get(i).copied()
    }

    fn char_at(&self, i: usize) -> Option<char> {
        self.source.get(i..).unwrap_unreachable2().chars().next()
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

    #[instrument(skip_all)]
    fn parse(&mut self) -> ParseResult<ProfileAttrBuilder> {
        self.skip_spaces();
        if !matches!(self.next(), Some(b'<')) {
            Ok(path_only_attr(normalize_path(self.source)?))
        } else {
            self.parse_xml_like()
        }
    }

    fn parse_xml_like(&mut self) -> ParseResult<ProfileAttrBuilder> {
        let mut attr = ProfileAttrBuilder::default();

        self.skip_spaces();
        match self.next_ident()? {
            "copy" => attr.ty = Some(AttrType::Copy),
            "link" => attr.ty = Some(AttrType::Link),
            "template" => attr.ty = Some(AttrType::Template),
            ty => return Err(format!("invalid type '{ty}'")),
        }

        loop {
            self.skip_spaces();
            match self.byte() {
                Some(ch) => match ch {
                    ident_pat!() => self.next_attribute(&mut attr)?,
                    b'>' => break,
                    _ => return Err(self.unexpected_char(self.index)),
                },
                None => return Err("unterminated angle brackets".to_owned()),
            }
        }
        Ok(attr)
    }

    fn next_attribute(&mut self, attr: &mut ProfileAttrBuilder) -> ParseResult<()> {
        macro_rules! check_dup {
            ($name:ident) => {
                if attr.$name.is_some() {
                    return Err(format!("duplicate '{}' attribute", stringify!($name)));
                }
            };
        }

        let key = self.next_ident()?;
        self.skip_spaces();
        let val = if matches!(self.byte(), Some(b'=')) {
            self.advance();
            self.skip_spaces();
            self.next_string()?
        } else {
            ""
        };
        match key {
            "recursive" => {
                check_dup!(recursive);
                match val {
                    "true" | "" => attr.recursive = Some(true),
                    "false" => attr.recursive = Some(false),
                    _ => return Err(format!("invalid 'recursive' value '{}'", val)),
                }
            }
            "source" => {
                check_dup!(source);
                attr.source = Some(normalize_path(val)?)
            }
            _ => warn!("Undefined attribute '{}'", key),
        }

        Ok(())
    }

    fn next_ident(&mut self) -> ParseResult<&'a str> {
        self.buf_start();
        self.expect(|ch| matches!(ch, ident_pat!()))?;
        while matches!(self.byte(), Some(ident_pat!() | b'0'..=b'9')) {
            self.advance();
        }
        Ok(self.buf_end())
    }

    fn next_string(&mut self) -> ParseResult<&'a str> {
        self.expect(|ch| ch == b'"')?;
        self.buf_start();
        while let Some(ch) = self.next() {
            if ch == b'"' {
                return Ok(self.buf_to(self.index - 1));
            }
        }
        Err("unterminated string".to_owned())
    }

    fn skip_spaces(&mut self) {
        while matches!(self.byte(), Some(b' ')) {
            self.advance();
        }
    }

    fn expect(&mut self, check: impl FnOnce(u8) -> bool) -> ParseResult<()> {
        match self.next() {
            Some(ch) => {
                if check(ch) {
                    Ok(())
                } else {
                    Err(self.unexpected_char(self.index - 1))
                }
            }
            None => Err("unexpected end of string".to_owned()),
        }
    }

    fn unexpected_char(&self, at: usize) -> String {
        format!(
            "unexpected character '{}' at {}",
            self.char_at(at).unwrap_unreachable2(),
            self.index,
        )
    }
}

pub fn normalize_path(path: &str) -> ParseResult<PathBuf> {
    let mut buf = PathBuf::new();
    for compo in path.as_ref2::<Path>().components() {
        match compo {
            Component::CurDir | Component::RootDir => (),
            Component::ParentDir => {
                buf.pop();
            }
            Component::Prefix(_) => return Err("a path can't start with a prefix".to_owned()),
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
            parse_attribute(r#"<link recursive="true">"#).unwrap(),
            expect_link_and_recursive(true)
        );
        assert_eq!(
            parse_attribute(r#"<link recursive="false">"#).unwrap(),
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
            parse_attribute("<link source>").unwrap(),
            expect_link_and_source("")
        );
        assert_eq!(
            parse_attribute(r#"<link source="path/to/source">"#).unwrap(),
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
        let s = r#"  <  template  recursive  =  "false"  source  =  "path/to/source"  >  "#;
        assert_eq!(parse_attribute(s).unwrap(), expected);
    }
}
