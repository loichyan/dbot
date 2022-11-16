use crate::{
    error,
    profile::{AttrType, Profile, ProfileAttr},
};
use std::{collections::HashMap, path::PathBuf};
use thisctx::WithContext;

pub type CompiledEntries = HashMap<PathBuf, CompiledProfile>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledProfile {
    pub source: PathBuf,
    pub ty: AttrType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerOptions {
    pub source: PathBuf,
    pub target: PathBuf,
}

pub fn compile(options: &CompilerOptions, profile: Profile) -> error::Result<CompiledEntries> {
    let entries = profile.into_entries()?;
    let mut compiled = CompiledEntries::default();
    // Compile child targets first to avoid double compiling.
    for (target, attr) in entries.into_iter().rev() {
        if cfg!(not(unix)) && matches!(attr.ty, AttrType::Link) {
            return None.context(error::UnsupportedSymlinksContext(attr.source));
        }
        compile_entry(
            options.target.join(&target),
            options.source.join(&attr.source),
            &attr,
            // TODO: copied source must be recursive
            attr.recursive || matches!(attr.ty, AttrType::Copy),
            &mut compiled,
        )?;
    }
    Ok(compiled)
}

fn compile_entry(
    target: PathBuf,
    mut source: PathBuf,
    attr: &ProfileAttr,
    recursive: bool,
    compiled: &mut CompiledEntries,
) -> error::Result<()> {
    // 1) Check whether compiled.
    if compiled.contains_key(&target) {
        return Ok(());
    }

    // 2) Resolve symlink.
    let metadata = source
        .metadata()
        .context(error::IoFailedContext { path: &source })?;
    if metadata.is_symlink() {
        source = std::fs::read_link(&source).context(error::IoFailedContext { path: &source })?;
    }

    // 3) Recursive compile entries under a directory.
    if metadata.is_dir() {
        if recursive {
            for entry in
                std::fs::read_dir(&source).context(error::IoFailedContext { path: &source })?
            {
                let entry = entry.context(error::IoFailedContext { path: &source })?;
                let filename = entry.file_name();
                if attr.ignore.is_match(&filename) {
                    continue;
                }
                compile_entry(
                    target.join(&filename),
                    source.join(&filename),
                    attr,
                    recursive,
                    compiled,
                )?;
            }
            return Ok(());
        } else if attr.ty == AttrType::Template {
            return ().context(error::UnexpectedDirectoryForTemplateContext(source));
        }
    }

    // 4) Insert current source.
    compiled.insert(
        target,
        CompiledProfile {
            source,
            ty: attr.ty,
        },
    );
    Ok(())
}

// TODO: test ignore patterns and overrided parent attributes
#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;
    use dotfish::DotFish;
    use std::path::Path;

    fn compile_str(source: &Path, profile: &str) -> error::Result<CompiledEntries> {
        let profile = serde_yaml::from_str::<Profile>(profile).unwrap();
        compile(
            &CompilerOptions {
                source: source.into(),
                target: "~".into(),
            },
            profile,
        )
    }

    fn create_tmp_tree(tmp: &Path) {
        create_tree!(tmp, {
            path: {
                to: {
                    source: {
                        file1,
                        file2,
                    },
                },
            },
        });
    }

    fn touched_file_entries(tmp: &Path, ty: AttrType) -> CompiledEntries {
        ["file1", "file2"]
            .iter()
            .map(|filename| {
                (
                    "~/path/to/target".as_ref2::<Path>().join(filename),
                    CompiledProfile {
                        source: tmp.join("path/to/source").join(filename),
                        ty,
                    },
                )
            })
            .collect::<CompiledEntries>()
    }

    #[test]
    fn copy_source() {
        let tempdir = tempfile::tempdir().unwrap();
        create_tmp_tree(tempdir.path());
        let entries = compile_str(
            tempdir.path(),
            r#"
            path/to/target: path/to/source
            "#,
        )
        .unwrap();
        let expected = touched_file_entries(tempdir.path(), AttrType::Copy);
        assert_eq!(entries, expected);
    }

    #[test]
    fn link_source() {
        let tempdir = tempfile::tempdir().unwrap();
        create_tmp_tree(tempdir.path());
        let entries = compile_str(
            tempdir.path(),
            r#"
            path/to/target: 
              ~source: path/to/source
              ~type: link
            "#,
        )
        .unwrap();
        let expected = std::iter::once((
            "~/path/to/target".into(),
            CompiledProfile {
                source: tempdir.path().join("path/to/source"),
                ty: AttrType::Link,
            },
        ))
        .collect::<CompiledEntries>();
        assert_eq!(entries, expected);
    }

    #[test]
    fn link_source_recursive() {
        let tempdir = tempfile::tempdir().unwrap();
        create_tmp_tree(tempdir.path());
        let entries = compile_str(
            tempdir.path(),
            r#"
            path/to/target:
              ~source: path/to/source
              ~type: link
              ~recursive: true
            "#,
        )
        .unwrap();
        let expected = touched_file_entries(tempdir.path(), AttrType::Link);
        assert_eq!(entries, expected);
    }

    #[test]
    fn template_source() {
        let tempdir = tempfile::tempdir().unwrap();
        create_tmp_tree(tempdir.path());
        let result = compile_str(
            tempdir.path(),
            r#"
            path/to/target: 
              ~source: path/to/source
              ~type: template
            "#,
        );
        assert!(
            matches!(
                &result,
                Err(error::Error::UnexpectedDirectoryForTemplate(path))
                    if path == &tempdir.path().join("path/to/source")
            ),
            "{result:?}"
        );
    }

    #[test]
    fn template_source_recursive() {
        let tempdir = tempfile::tempdir().unwrap();
        create_tmp_tree(tempdir.path());
        let entries = compile_str(
            tempdir.path(),
            r#"
            path/to/target:
              ~source: path/to/source
              ~type: template
              ~recursive: true
            "#,
        )
        .unwrap();
        let expected = touched_file_entries(tempdir.path(), AttrType::Template);
        assert_eq!(entries, expected);
    }
}
