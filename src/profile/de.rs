use super::{normalize_path, path_only_node, AttrLink, ProfileAttr, ProfileNode};
use serde::{
    de::{Error as DeError, Visitor},
    Deserialize,
};
use std::{fmt, path::PathBuf, rc::Rc};
use tracing::{instrument, warn};

fn deserialize_path_normalized<E: DeError>(v: &str) -> Result<PathBuf, E> {
    normalize_path(v.as_ref()).map_err(E::custom)
}

struct AttrLinkVisitor;

impl<'de> Visitor<'de> for AttrLinkVisitor {
    type Value = AttrLink;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a link attribute")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        match v {
            "recursive" => Ok(AttrLink::Recursive),
            _ => Err(E::custom("unexpected value")),
        }
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        match v {
            true => Ok(AttrLink::True),
            false => Ok(AttrLink::False),
        }
    }
}

impl<'de> Deserialize<'de> for AttrLink {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(AttrLinkVisitor)
    }
}

struct ProfileNodeVistor;

impl<'de> Visitor<'de> for ProfileNodeVistor {
    type Value = ProfileNode;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a profile")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        deserialize_path_normalized(v.as_ref()).map(path_only_node)
    }

    #[instrument(skip_all)]
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut attr = ProfileAttr::default();
        let mut children = Vec::with_capacity(map.size_hint().unwrap_or_default());
        while let Some(key) = map.next_key::<&str>()? {
            if key.starts_with("~") {
                match &key[1..] {
                    "from" => attr.from = Some(deserialize_path_normalized(map.next_value()?)?),
                    "link" => attr.link = Some(map.next_value()?),
                    "tmpl" => attr.tmpl = Some(map.next_value()?),
                    "ignore" => attr.ignore = Some(Rc::new(map.next_value()?)),
                    _ => {
                        warn!("Undefined attribute '{}'", key);
                        map.next_value::<serde_yaml::Value>()?;
                    }
                }
            } else {
                let dest = deserialize_path_normalized(key)?;
                let node = map.next_value::<ProfileNode>()?;
                children.push((dest, node));
            }
        }
        Ok(ProfileNode { attr, children })
    }
}

impl<'de> Deserialize<'de> for ProfileNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ProfileNodeVistor)
    }
}

#[cfg(test)]
mod tests {
    use crate::profile::path_only_attr;

    use super::*;

    #[test]
    fn deserialize_attr_link() {
        assert_eq!(
            serde_yaml::from_str::<AttrLink>("true").unwrap(),
            AttrLink::True
        );
        assert_eq!(
            serde_yaml::from_str::<AttrLink>("false").unwrap(),
            AttrLink::False
        );
        assert_eq!(
            serde_yaml::from_str::<AttrLink>("recursive").unwrap(),
            AttrLink::Recursive
        );
    }

    #[test]
    fn normalized_path_attributes() {
        let node = serde_yaml::from_str::<ProfileNode>(
            r#"
            ~from: skip/../path/to/root
            /path/to/./target1: ../path/to/source1
            "#,
        )
        .unwrap();
        let expected = ProfileNode {
            attr: path_only_attr("path/to/root"),
            children: vec![("path/to/target1".into(), path_only_node("path/to/source1"))],
        };
        assert_eq!(node, expected);
    }

    #[test]
    fn deserialize_profile_attr() {
        let node = serde_yaml::from_str::<ProfileNode>(
            r#"
            ~from: path/to/source
            ~link: true
            ~undefined_attr: ...
            ~tmpl: true
            "#,
        )
        .unwrap();
        let attr = ProfileAttr {
            from: Some("path/to/source".into()),
            link: AttrLink::True.into(),
            tmpl: true.into(),
            ignore: <_>::default(),
        };
        assert_eq!(node.attr, attr);
        assert!(node.children.is_empty());
    }

    #[test]
    fn deserialize_profile_node() {
        let node = serde_yaml::from_str::<ProfileNode>(
            r#"
            target1: path/to/source1
            target2:
              ~from: path/to/source2
            "#,
        )
        .unwrap();
        let child1 = path_only_node("path/to/source1");
        let child2 = path_only_node("path/to/source2");
        let expected = ProfileNode {
            children: vec![
                ("target1".to_owned().into(), child1),
                ("target2".to_owned().into(), child2),
            ],
            ..Default::default()
        };
        assert_eq!(node, expected);
    }
}
