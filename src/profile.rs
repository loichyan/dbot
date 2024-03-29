mod de;
mod parse;

use crate::{
    error,
    merge::Merge,
    pattern::{PatternSet, PatternSetBuilder},
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
    rc::Rc,
};
use thisctx::{IntoError, WithContext};

fn rc_unwrap_or_clone<T: Clone>(rc: Rc<T>) -> T {
    Rc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone())
}

fn extend_set_build(
    this: Rc<CachedPatternSetBuilder>,
    parent: &CachedPatternSetBuilder,
) -> Rc<CachedPatternSetBuilder> {
    let mut this = rc_unwrap_or_clone(this);
    this.builder.extend(parent.builder.iter().cloned());
    Rc::new(this)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(transparent)]
pub struct Profile {
    root: ProfileNode,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct ProfileEntries(pub(crate) Vec<(PathBuf, ProfileAttr)>);

impl Merge for Profile {
    fn merge(&mut self, other: Profile) {
        self.root.merge(other.root);
    }
}

impl Profile {
    pub fn into_entries(mut self) -> error::Result<ProfileEntries> {
        let mut collect_to = ProfileEntries(<_>::default());
        let node = self.build_component_tree();
        collect_entries_from_node(
            "".as_ref(),
            node,
            "".as_ref(),
            &<_>::default(),
            &mut collect_to,
        )?;
        collect_to.0.sort_by(|(a, _), (b, _)| a.cmp(b));
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
) -> error::Result<()> {
    let ComponentNode { mut attr, children } = node;
    let full_target = parent_target.join(target);

    // 1) Inherit attribute.
    attr = inherit_attr(target, attr, parent)?;

    // 2) Validate attribute.
    if matches!(attr.ty, Some(AttrType::Template | AttrType::Link))
        && !matches!(attr.recursive, Some(true))
        && !children.is_empty()
    {
        return error::UnexpectedChildren(full_target).fail();
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
        collect_to.0.push((full_target, attr));
    }
    Ok(())
}

fn inherit_attr(
    target: &Path,
    attr: ProfileAttrBuilder,
    parent: &ProfileAttrBuilder,
) -> error::Result<ProfileAttrBuilder> {
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
        (Some(this), Some(parent)) => Some(extend_set_build(this, &parent)),
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
    tree.attr.merge(std::mem::take(&mut node.attr));
    for (child_target, child_node) in node.children.iter_mut() {
        update_component_tree(child_target, child_node, tree);
    }
}

impl Merge for ProfileAttrBuilder {
    fn merge(&mut self, other: ProfileAttrBuilder) {
        macro_rules! merge_field {
            ($($field:ident),*) => {$(
                self.$field.merge(other.$field);
            )*};
        }
        merge_field!(source, ty, recursive);
        if let Some(other) = other.ignore {
            if !other.builder.is_empty() {
                let mut this = self
                    .ignore
                    .take()
                    .map(rc_unwrap_or_clone)
                    .unwrap_or_default();
                match Rc::try_unwrap(other) {
                    Ok(t) => this.builder.extend(t.builder.into_iter()),
                    Err(rc) => this.builder.extend(rc.builder.iter().cloned()),
                }
                self.ignore = Some(Rc::new(this));
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ProfileAttrBuilder {
    source: Option<PathBuf>,
    ty: Option<AttrType>,
    recursive: Option<bool>,
    ignore: Option<Rc<CachedPatternSetBuilder>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(transparent)]
struct CachedPatternSetBuilder {
    builder: PatternSetBuilder,
    #[serde(skip)]
    cache: OnceCell<Rc<PatternSet>>,
}

impl PartialEq for CachedPatternSetBuilder {
    fn eq(&self, other: &Self) -> bool {
        self.builder.eq(&other.builder)
    }
}

impl Eq for CachedPatternSetBuilder {}

impl CachedPatternSetBuilder {
    fn build(&self) -> Option<Rc<PatternSet>> {
        self.cache
            .get_or_try_init(|| self.builder.build().map(Rc::new))
            .cloned()
            .ok()
    }
}

impl ProfileAttrBuilder {
    fn build(self, target: &Path) -> error::Result<Option<ProfileAttr>> {
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
                    Some(builder) => builder.build().context(error::InvalidPatternSet(target))?,
                    None => <_>::default(),
                },
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq,))]
pub struct ProfileAttr {
    pub source: PathBuf,
    pub ty: AttrType,
    pub recursive: bool,
    pub ignore: Rc<PatternSet>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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
    children: HashMap<PathBuf, ProfileNode>,
}

impl Merge for ProfileNode {
    fn merge(&mut self, other: Self) {
        self.attr.merge(other.attr);
        self.children.merge(other.children);
    }
}

#[allow(dead_code)]
fn path_only_attr<T>(source: T) -> ProfileAttrBuilder
where
    T: Into<PathBuf>,
{
    ProfileAttrBuilder {
        source: Some(source.into()),
        ..Default::default()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ComponentNode<'a> {
    attr: ProfileAttrBuilder,
    children: HashMap<&'a OsStr, ComponentNode<'a>>,
}

// TODO: test merge and inherit ignore patterns
#[cfg(test)]
mod tests {
    use super::*;

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

    fn profile_from_str(s: &str) -> Profile {
        serde_yaml::from_str(s).unwrap()
    }

    #[test]
    fn build_component_tree() {
        let mut profile = profile_from_str(
            r#"
            path:
              to:
                target1: path/to/source1
              to/target2: path/to/source2
            path/to/target3: path/to/source3
            "#,
        );
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
        let mut profile = profile_from_str(
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
        );
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
        let entries = profile_from_str(
            r#"
            path:
              to:
                target1: path/to/source1
              to/target2: path/to/source2
            path/to/target3: path/to/source3
            "#,
        )
        .into_entries()
        .unwrap();
        let expected = ProfileEntries(
            [
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
            .collect(),
        );
        assert_eq!(entries, expected);
    }

    #[test]
    fn inherited_from_attribute() {
        let entries = profile_from_str(
            r#"
            path/to/target:
              ~source: path/to/source
              child1: path/to/child1
              child2:
                ~type: link
            "#,
        )
        .into_entries()
        .unwrap();
        let mut expected = ProfileEntries(
            [
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
            .collect(),
        );
        expected.0.push((
            "path/to/target/child2".into(),
            ProfileAttr {
                source: "path/to/source/child2".into(),
                ty: AttrType::Link,
                recursive: false,
                ignore: <_>::default(),
            },
        ));
        assert_eq!(entries, expected);
    }

    fn test_into_entries_error(s: &str, f: impl FnOnce(error::Error) -> bool) {
        let result = profile_from_str(s).into_entries();
        assert!(f(result.unwrap_err()))
    }

    fn expects_unexpected_children(path: &Path) -> impl '_ + FnOnce(error::Error) -> bool {
        move |err| matches!(err, error::Error::UnexpectedChildren(p) if p == path)
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
            expects_unexpected_children("path/to/target".as_ref()),
        );
        test_into_entries_error(
            r#"
            path/to/target:
              ~source: path/to/source
              ~type: template
              ~recursive: false
              child1: path/to/child1
            "#,
            expects_unexpected_children("path/to/target".as_ref()),
        );
    }
}
