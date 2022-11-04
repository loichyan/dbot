use super::{AttrLink, ProfileAttr, ProfileNode};
use serde::{
    de::{Error as DeError, Visitor},
    Deserialize,
};
use std::fmt;
use tracing::{instrument, warn};

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

fn path_only_node(from: String) -> ProfileNode {
    ProfileNode {
        attr: ProfileAttr {
            from: Some(from),
            ..Default::default()
        },
        ..Default::default()
    }
}

impl<'de> Visitor<'de> for ProfileNodeVistor {
    type Value = ProfileNode;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a profile")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_string(v.to_owned())
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(path_only_node(v))
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
                    "from" => attr.from = Some(map.next_value()?),
                    "link" => attr.link = Some(map.next_value()?),
                    "tmpl" => attr.tmpl = Some(map.next_value()?),
                    "ignore" => attr.ignore = Some(map.next_value()?),
                    _ => {
                        warn!("Undefined attribute '{}'", key);
                        map.next_value::<serde_yaml::Value>()?;
                    }
                }
            } else {
                let dest = key.to_owned();
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
            from: "path/to/source".to_owned().into(),
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
        let child1 = path_only_node("path/to/source1".to_owned());
        let child2 = path_only_node("path/to/source2".to_owned());
        let expected = ProfileNode {
            children: vec![
                ("target1".to_owned(), child1),
                ("target2".to_owned(), child2),
            ],
            ..Default::default()
        };
        assert_eq!(node, expected);
    }
}
