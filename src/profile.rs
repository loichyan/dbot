mod de;

use crate::{
    error::Error,
    pattern::{PatternSet, PatternSetBuilder},
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
    rc::Rc,
};

pub type ProfileEntries = HashMap<PathBuf, ProfileAttr>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(transparent)]
pub struct Profile {
    root: ProfileNode,
}

impl Profile {
    pub fn into_entries(mut self) -> Result<ProfileEntries, Error> {
        let mut collect_to = HashMap::default();
        let node = self.build_component_tree();
        collect_entries_from_node(
            "".as_ref(),
            node,
            "".as_ref(),
            &<_>::default(),
            &mut collect_to,
        )?;
        Ok(collect_to)
    }

    /// Splits target paths into components and constructs a attributes tree.
    ///
    /// Some nodes' parents may not be their parents in the filesystem, and
    /// the component tree avoids this problem, so this is a necessary step
    /// in order to properly handle the attribute inheritance relationships
    /// during [`collect_entries`].
    fn build_component_tree(&mut self) -> ComponentNode<'_> {
        let mut tree = ComponentNode::default();
        update_component_tree("".as_ref(), &mut self.root, &mut tree);
        tree
    }
}

fn collect_entries_from_node(
    target: &Path,
    node: ComponentNode<'_>,
    parent_target: &Path,
    parent: &ProfileAttrBuilder,
    collect_to: &mut ProfileEntries,
) -> Result<(), Error> {
    let ComponentNode { mut attr, children } = node;
    let full_target = parent_target.join(target);

    // 1) Inherit attribute.
    attr = inherit_attr(target, attr, parent)?;

    // 2) Validate attribute.
    if matches!(attr.ty, Some(AttrType::Template | AttrType::Link))
        && !matches!(attr.recursive, Some(true))
        && !children.is_empty()
    {
        return Err(Error::UnexpectedChildren(full_target));
    }

    // 3) Collect from child nodes.
    for (child_target, child_node) in children {
        collect_entries_from_node(
            child_target.as_ref(),
            child_node,
            &full_target,
            &attr,
            collect_to,
        )?;
    }

    // 4) Build attribute and collect to entries.
    // Ignore entries without `source` attribute.
    if let Some(attr) = attr.build(&full_target)? {
        collect_to.insert(full_target, attr);
    }
    Ok(())
}

fn inherit_attr(
    target: &Path,
    attr: ProfileAttrBuilder,
    parent: &ProfileAttrBuilder,
) -> Result<ProfileAttrBuilder, Error> {
    // Attributes to keep.
    let source = attr
        .source
        // By default, if a child under a node that provides `source` attribute
        // doesn't define its own `source` attribute, it will be treated a child
        // of the directory whose parent `source` path it is.
        .or_else(|| parent.source.as_ref().map(|parent| parent.join(target)));
    // Attributes to override.
    let ty = attr.ty.or(parent.ty);
    let recursive = attr.recursive.or(parent.recursive);
    // Attributes to extend.
    let ignore = match (attr.ignore, parent.ignore.clone()) {
        (Some(this), Some(parent)) => Some(Rc::new(parent.extend(&this))),
        (Some(val), _) | (_, Some(val)) => Some(val),
        _ => None,
    };
    Ok(ProfileAttrBuilder {
        source,
        ty,
        recursive,
        ignore,
    })
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
fn merge_attr(dest: &mut ProfileAttrBuilder, src: ProfileAttrBuilder) {
    macro_rules! merge_attr_fields {
        ($($field:ident),*) => {
            // Ensures fields are exhausted.
            const _: () = {
                ProfileAttrBuilder {
                    $($field: None,)*
                };
            };
            $(src.$field.map(|v| dest.$field.insert(v));)*
        };
    }
    merge_attr_fields!(source, ty, recursive, ignore);
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ProfileAttrBuilder {
    source: Option<PathBuf>,
    ty: Option<AttrType>,
    recursive: Option<bool>,
    ignore: Option<Rc<PatternSetBuilder>>,
}

impl ProfileAttrBuilder {
    fn build(self, target: &Path) -> Result<Option<ProfileAttr>, Error> {
        let ProfileAttrBuilder {
            source,
            ty,
            recursive,
            ignore,
        } = self;
        if let Some(source) = source {
            Ok(Some(ProfileAttr {
                source,
                ty: ty.unwrap_or_default(),
                recursive: recursive.unwrap_or_default(),
                ignore: match ignore {
                    Some(builder) => Rc::try_unwrap(builder)
                        .map(|builder| builder.build())
                        .unwrap_or_else(|builder| (*builder).clone().build())
                        .ok_or_else(|| Error::InvalidPatternSet(target.into()))?,
                    None => <_>::default(),
                },
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg_attr(test, derive(Eq, PartialEq,))]
#[derive(Clone, Debug, Default)]
pub struct ProfileAttr {
    pub source: PathBuf,
    pub ty: AttrType,
    pub recursive: bool,
    pub ignore: PatternSet,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(expecting = "a type attribute", rename_all = "lowercase")]
pub enum AttrType {
    #[default]
    Copy,
    Link,
    Template,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ProfileNode {
    attr: ProfileAttrBuilder,
    children: Vec<(PathBuf, ProfileNode)>,
}

fn path_only_attr<T>(source: T) -> ProfileAttrBuilder
where
    T: Into<PathBuf>,
{
    ProfileAttrBuilder {
        source: Some(source.into()),
        ..Default::default()
    }
}

fn path_only_node<T>(source: T) -> ProfileNode
where
    T: Into<PathBuf>,
{
    ProfileNode {
        attr: path_only_attr(source.into()),
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ComponentNode<'a> {
    attr: ProfileAttrBuilder,
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

    fn path_only_component_node<'a, T>(source: T) -> ComponentNode<'a>
    where
        T: Into<PathBuf>,
    {
        ComponentNode {
            attr: path_only_attr(source),
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
                  ~source: path/to/source1
                  ~type: link
              to/target1:
                  ~type: template
            path/to/target1:
              ~type: link
              ~recursive: true
            "#,
        )
        .unwrap();
        let attr = ProfileAttrBuilder {
            source: Some("path/to/source1".into()),
            ty: Some(AttrType::Link),
            recursive: Some(true),
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
        let entries = serde_yaml::from_str::<Profile>(
            r#"
            path:
              to:
                target1: path/to/source1
              to/target2: path/to/source2
            path/to/target3: path/to/source3
            "#,
        )
        .unwrap()
        .into_entries()
        .unwrap();
        let expected = [
            ("path/to/target1", "path/to/source1"),
            ("path/to/target2", "path/to/source2"),
            ("path/to/target3", "path/to/source3"),
        ]
        .into_iter()
        .map(|(target, source)| {
            (
                PathBuf::from(target),
                path_only_attr(source)
                    .build(target.as_ref())
                    .unwrap()
                    .unwrap(),
            )
        })
        .collect::<HashMap<_, _>>();
        assert_eq!(entries, expected);
    }

    #[test]
    fn inherited_from_attribute() {
        let entries = serde_yaml::from_str::<Profile>(
            r#"
            path/to/target:
              ~source: path/to/source
              child1: path/to/child1
              child2:
                ~type: link
            "#,
        )
        .unwrap()
        .into_entries()
        .unwrap();
        let mut expected = [
            ("path/to/target", "path/to/source"),
            ("path/to/target/child1", "path/to/child1"),
        ]
        .into_iter()
        .map(|(target, source)| {
            (
                PathBuf::from(target),
                path_only_attr(source)
                    .build(target.as_ref())
                    .unwrap()
                    .unwrap(),
            )
        })
        .collect::<HashMap<_, _>>();
        expected.insert(
            "path/to/target/child2".into(),
            ProfileAttr {
                source: "path/to/source/child2".into(),
                ty: AttrType::Link,
                ..Default::default()
            },
        );
        assert_eq!(entries, expected);
    }

    fn test_into_entries_error(s: &str, err: Error) {
        let result = serde_yaml::from_str::<Profile>(s).unwrap().into_entries();
        assert_eq!(result, Err(err))
    }

    #[test]
    fn unexpected_children() {
        test_into_entries_error(
            r#"
            path/to/target:
              ~source: path/to/source
              ~type: link
              child1: path/to/child1
            "#,
            Error::UnexpectedChildren("path/to/target".into()),
        );
        test_into_entries_error(
            r#"
            path/to/target:
              ~source: path/to/source
              ~type: template
              ~recursive: false
              child1: path/to/child1
            "#,
            Error::UnexpectedChildren("path/to/target".into()),
        );
    }
}
