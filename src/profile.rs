mod de;

use crate::pattern::PatternSetBuilder;
use serde::Deserialize;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
    rc::Rc,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct Profile {
    root: ProfileNode,
}

impl Profile {
    pub fn into_entries(mut self) -> HashMap<PathBuf, ProfileAttr> {
        let ComponentNode { attr, children } = self.build_component_tree();
        let mut collect_to = HashMap::default();
        for (child_target, child_node) in children {
            collect_entries_from_node(
                child_target.as_ref(),
                child_node,
                "".as_ref(),
                &attr,
                &mut collect_to,
            );
        }
        collect_to
    }

    /// Splits target paths into components and constructs a attributes tree.
    ///
    /// Some nodes' parents may not be their parents in the filesystem, and
    /// the component tree avoids this problem, so this is a necessary step
    /// in order to properly handle the attribute inheritance relationships
    /// during [`collect_entries`].
    fn build_component_tree(&mut self) -> ComponentNode<'_> {
        let node = &mut self.root;
        let mut tree = ComponentNode::default();
        for (child_target, child_node) in node.children.iter_mut() {
            update_component_tree(child_target, child_node, &mut tree);
        }
        tree
    }
}

fn collect_entries_from_node(
    target: &Path,
    node: ComponentNode<'_>,
    parent_target: &Path,
    parent: &ProfileAttr,
    collect_to: &mut HashMap<PathBuf, ProfileAttr>,
) {
    let ComponentNode { mut attr, children } = node;
    attr = inherit_attr(target, attr, parent);
    let target = parent_target.join(target);
    for (child_target, child_node) in children {
        collect_entries_from_node(
            child_target.as_ref(),
            child_node,
            &target,
            &attr,
            collect_to,
        );
    }
    // Ignore entries without 'from' attribute.
    if attr.from.is_none() {
        return;
    }
    collect_to.insert(target, attr);
}

// TODO: check attributes which are impossible to compile
fn inherit_attr(target: &Path, attr: ProfileAttr, parent: &ProfileAttr) -> ProfileAttr {
    // Attributes to keep.
    let from = attr
        .from
        // By default, if a child under a node that provides `from` attribute
        // doesn't define its own `from` attribute, it will be treated a child
        // of the directory whose parent `from` path it is.
        .or_else(|| parent.from.as_ref().map(|parent| parent.join(&target)));
    // Attributes to override.
    // TODO: a template or linked path cannot have children
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

fn update_component_tree<'a>(
    target: &'a Path,
    node: &'a mut ProfileNode,
    mut tree: &mut ComponentNode<'a>,
) {
    // Update `tree` to the last component of `target`.
    for compo in target.components() {
        match compo {
            Component::Normal(compo) => tree = tree.children.entry(compo).or_default(),
            _ => unreachable!(),
        }
    }
    merge_attr(&mut tree.attr, std::mem::take(&mut node.attr));
    for (child_target, child_node) in node.children.iter_mut() {
        update_component_tree(child_target, child_node, tree);
    }
}

/// Merges attributes of `src` into `dest`.
fn merge_attr(dest: &mut ProfileAttr, src: ProfileAttr) {
    macro_rules! merge_attr_fields {
        ($($field:ident),*) => {
            // Ensures fields are exhausted.
            const _: () = {
                ProfileAttr {
                    $($field: None,)*
                };
            };
            $(src.$field.map(|v| dest.$field.insert(v));)*
        };
    }
    merge_attr_fields!(from, link, tmpl, ignore);
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileAttr {
    pub from: Option<PathBuf>,
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
    children: Vec<(PathBuf, ProfileNode)>,
}

fn path_only_attr<T>(from: T) -> ProfileAttr
where
    T: Into<PathBuf>,
{
    ProfileAttr {
        from: Some(from.into()),
        ..Default::default()
    }
}

fn path_only_node<T>(from: T) -> ProfileNode
where
    T: Into<PathBuf>,
{
    ProfileNode {
        attr: path_only_attr(from.into()),
        ..Default::default()
    }
}

fn normalize_path(path: &Path) -> Result<PathBuf, &'static str> {
    let mut buf = PathBuf::new();
    for compo in path.components() {
        match compo {
            Component::CurDir | Component::RootDir => (),
            Component::ParentDir => {
                buf.pop();
            }
            Component::Prefix(_) => return Err("a path can't start with a prefix"),
            _ => buf.push(compo),
        }
    }
    if buf.as_os_str().is_empty() {
        return Err("a normalized path must not be empty");
    }
    Ok(buf)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ComponentNode<'a> {
    attr: ProfileAttr,
    children: HashMap<&'a OsStr, ComponentNode<'a>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_normalized_path() {
        assert_eq!(
            PathBuf::from("abc/def"),
            normalize_path("skip/../abc/def".as_ref()).unwrap()
        );
        assert_eq!(
            PathBuf::from("abc/def"),
            normalize_path("/abc/def".as_ref()).unwrap()
        );
        assert_eq!(
            PathBuf::from("abc/def"),
            normalize_path("abc/./def".as_ref()).unwrap()
        );
    }

    fn create_component_node<'a, I>(entries: I) -> ComponentNode<'a>
    where
        I: IntoIterator<Item = (&'a str, ComponentNode<'a>)>,
    {
        let children = entries
            .into_iter()
            .map(|(target, node)| (target.as_ref(), node))
            .collect();
        ComponentNode {
            children,
            ..Default::default()
        }
    }

    fn path_only_component_node<'a, T>(from: T) -> ComponentNode<'a>
    where
        T: Into<PathBuf>,
    {
        ComponentNode {
            attr: path_only_attr(from),
            ..Default::default()
        }
    }

    #[test]
    fn build_component_tree() {
        let mut profile = serde_yaml::from_str::<Profile>(
            r#"
            path:
              to:
                target1: path/to/source1
              to/target2: path/to/source2
            path/to/target3: path/to/source3
            "#,
        )
        .unwrap();
        let children = create_component_node(
            [
                ("target1", "path/to/source1"),
                ("target2", "path/to/source2"),
                ("target3", "path/to/source3"),
            ]
            .into_iter()
            .map(|(target, source)| (target, path_only_component_node(source))),
        );
        let expected = create_component_node([("path", create_component_node([("to", children)]))]);
        assert_eq!(profile.build_component_tree(), expected);
    }

    #[test]
    fn merge_attributes() {
        let mut profile = serde_yaml::from_str::<Profile>(
            r#"
            path:
              to:
                target1:
                  ~from: path/to/source1
                  ~link: true
              to/target1:
                  ~tmpl: true
            path/to/target1:
              ~link: recursive
              ~tmpl: false
            "#,
        )
        .unwrap();
        let attr = ProfileAttr {
            from: Some("path/to/source1".into()),
            link: Some(AttrLink::Recursive),
            tmpl: Some(false),
            ignore: None,
        };
        let expected = create_component_node([(
            "path",
            create_component_node([(
                "to",
                create_component_node([(
                    "target1",
                    ComponentNode {
                        attr,
                        ..Default::default()
                    },
                )]),
            )]),
        )]);
        assert_eq!(profile.build_component_tree(), expected);
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
        let expected = [
            ("path/to/target1", "path/to/source1"),
            ("path/to/target2", "path/to/source2"),
            ("path/to/target3", "path/to/source3"),
        ]
        .into_iter()
        .map(|(target, source)| (PathBuf::from(target), path_only_attr(source)))
        .collect::<HashMap<_, _>>();
        assert_eq!(profile.into_entries(), expected);
    }

    #[test]
    fn inherited_from_attribute() {
        let entries = serde_yaml::from_str::<Profile>(
            r#"
            path/to/target:
              ~from: path/to/source
              child1: path/to/child1
              child2:
                ~link: true
            "#,
        )
        .unwrap()
        .into_entries();
        let mut expected = [
            ("path/to/target", "path/to/source"),
            ("path/to/target/child1", "path/to/child1"),
        ]
        .into_iter()
        .map(|(target, source)| (PathBuf::from(target), path_only_attr(source)))
        .collect::<HashMap<_, _>>();
        expected.insert(
            "path/to/target/child2".into(),
            ProfileAttr {
                from: Some("path/to/source/child2".into()),
                link: Some(AttrLink::True),
                ..Default::default()
            },
        );
        assert_eq!(entries, expected);
    }
}
