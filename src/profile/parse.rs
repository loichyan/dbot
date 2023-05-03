use super::{AttrType, ProfileAttrBuilder};
use std::path::{Component, Path, PathBuf};
use thisctx::IntoError;

mod error {
    use thisctx::WithContext;
    use thiserror::Error;

    #[derive(Debug, Error, WithContext)]
    pub(crate) enum Error<'a> {
        #[error("a path can't start with '{0}' or other prefixes")]
        PathStartsWithPrefix(&'a str),
        #[error("profile type must end with '>'")]
        TypeMustEndWithGt,
        #[error("unknown attribute type '{0}'")]
        UnknownType(&'a str),
    }
}

type Result<'a, T> = std::result::Result<T, error::Error<'a>>;

macro_rules! define_ty {
    ($($name:ident => $variant:ident,)*) => {
        const ALL_TYPES: &[(&str, AttrType)] = &[$((stringify!($name), AttrType::$variant),)*];
    };
}

define_ty! {
    copy     => Copy,
    link     => Link,
    template => Template,
}

pub(super) fn parse_attribute(s: &str) -> Result<ProfileAttrBuilder> {
    Ok(if s.starts_with('<') {
        if s.ends_with('>') {
            let ty = &s[1..s.len() - 1];
            ProfileAttrBuilder {
                ty: if let Ok(i) = ALL_TYPES.binary_search_by(|(s, _)| s.cmp(&ty)) {
                    Some(ALL_TYPES[i].1)
                } else {
                    return error::UnknownType(s).fail();
                },
                ..Default::default()
            }
        } else {
            return error::TypeMustEndWithGt.fail();
        }
    } else {
        ProfileAttrBuilder {
            source: normalize_path(s).map(Some)?,
            ..Default::default()
        }
    })
}

pub(crate) fn normalize_path(path: &str) -> Result<PathBuf> {
    let mut buf = PathBuf::new();
    for compo in Path::new(path).components() {
        match compo {
            Component::CurDir | Component::RootDir => (),
            Component::ParentDir => {
                buf.pop();
            }
            Component::Prefix(a) => {
                return error::PathStartsWithPrefix(a.as_os_str().to_str().unwrap()).fail()
            }
            _ => buf.push(compo),
        }
    }
    Ok(buf)
}
