mod de;

use crate::pattern::PatternSetBuilder;
use indexmap::IndexMap;
use serde::Deserialize;
use std::{
    path::{Component, Path, PathBuf},
    rc::Rc,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct Profile {
    root: ProfileNode,
}

impl Profile {
    pub fn collect_entries(self) -> IndexMap<PathBuf, ProfileAttr> {
        let Self {
            root: ProfileNode { attr, children },
        } = self;
        let mut collect_to = IndexMap::default();
        for (child_target, child_node) in children {
            collect_entries_from_node(child_target.0, child_node, &attr, &mut collect_to);
        }
        collect_to
    }
}

fn collect_entries_from_node(
    target: PathBuf,
    node: ProfileNode,
    parent: &ProfileAttr,
    collect_to: &mut IndexMap<PathBuf, ProfileAttr>,
) {
    let ProfileNode { mut attr, children } = node;
    attr = inherit_attr(attr, parent);
    for (child_target, child_node) in children {
        collect_entries_from_node(target.join(child_target.0), child_node, &attr, collect_to);
    }
    // Ignore entries without 'from' attribute.
    if attr.from.is_none() {
        return;
    }
    let attr = if let Some(prev_attr) = collect_to.get(&target) {
        // Inherit existing attributes.
        inherit_attr(attr, prev_attr)
    } else {
        attr
    };
    collect_to.insert(target, attr);
}

// TODO: check attributes which are impossible to compile
fn inherit_attr(attr: ProfileAttr, parent: &ProfileAttr) -> ProfileAttr {
    // Attributes to keep.
    let from = attr.from;
    // Attributes to override.
    let link = attr.link.or(parent.link);
    let tmpl = attr.tmpl.or(parent.tmpl);
    // Attributes to extend.
    let ignore = match (attr.ignore, parent.ignore.clone()) {
        (Some(this), Some(parent)) => Some(Rc::new(parent.extend(&this))),
        (Some(val), _) | (_, Some(val)) => Some(val),
        _ => None,
    };
    ProfileAttr {
        from,
        link,
        tmpl,
        ignore,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileAttr {
    pub from: Option<NormalizedPath>,
    pub link: Option<AttrLink>,
    pub tmpl: Option<bool>,
    pub ignore: Option<Rc<PatternSetBuilder>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AttrLink {
    #[default]
    False,
    True,
    Recursive,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ProfileNode {
    attr: ProfileAttr,
    children: Vec<(NormalizedPath, ProfileNode)>,
}

fn path_only_node<T>(from: T) -> ProfileNode
where
    T: Into<NormalizedPath>,
{
    ProfileNode {
        attr: ProfileAttr {
            from: Some(from.into()),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NormalizedPath(PathBuf);

impl<T> From<T> for NormalizedPath
where
    T: Into<PathBuf>,
{
    fn from(t: T) -> Self {
        Self(t.into())
    }
}

impl NormalizedPath {
    fn new(path: &Path) -> Self {
        let mut buf = PathBuf::new();
        for compo in path.components() {
            match compo {
                Component::CurDir => (),
                Component::ParentDir => {
                    buf.pop();
                }
                _ => buf.push(compo),
            }
        }
        Self(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path() {
        assert_eq!(
            PathBuf::from("abc/def"),
            NormalizedPath::new("./../skip/../abc/./skip/../def".as_ref()).0
        )
    }

    #[test]
    fn collect_entries() {
        let profile = serde_yaml::from_str::<Profile>(
            r#"
            path:
              to:
                target1: path/to/source1
              to/target2: path/to/source2
            path/to/target3: path/to/source3
            "#,
        )
        .unwrap();
        let child1 = path_only_node("path/to/source1");
        let child2 = path_only_node("path/to/source2");
        let child3 = path_only_node("path/to/source3");
        let expected = vec![
            ("path/to/target1", child1),
            ("path/to/target2", child2),
            ("path/to/target3", child3),
        ]
        .into_iter()
        .map(|(target, node)| (PathBuf::from(target), node.attr))
        .collect::<IndexMap<_, _>>();
        assert_eq!(profile.collect_entries(), expected);
    }
}
