use crate::{
    error,
    profile::{AttrType, Profile, ProfileAttr},
};
use std::{collections::HashMap, path::PathBuf};
use thisctx::{IntoError, WithContext};

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
            return error::UnsupportedSymlinks(attr.source).fail();
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
        .context(error::IoFailed { path: &source })?;
    if metadata.is_symlink() {
        source = std::fs::read_link(&source).context(error::IoFailed { path: &source })?;
    }

    // 3) Recursive compile entries under a directory.
    if metadata.is_dir() {
        if recursive {
            for entry in std::fs::read_dir(&source).context(error::IoFailed { path: &source })? {
                let entry = entry.context(error::IoFailed { path: &source })?;
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
            return error::UnexpectedDirectoryForTemplate(source).fail();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;
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
        compiled_entries(tmp, &["file1", "file2"], ty)
    }

    fn compiled_entries(tmp: &Path, paths: &[&str], ty: AttrType) -> CompiledEntries {
        paths
            .iter()
            .map(|filename| {
                (
                    Path::new("~/path/to/target").join(filename),
                    CompiledProfile {
                        source: tmp.join("path/to/source").join(filename),
                        ty,
                    },
                )
            })
            .collect()
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

    #[test]
    fn ignore() {
        let tempdir = tempfile::tempdir().unwrap();
        create_tree!(tempdir.path(), {
            path: {
                to: {
                    source: {
                        file1,
                        file2,
                        ignore1,
                        ignore2,
                        ignore_dir: {
                            file1,
                            file2,
                        },
                    },
                },
            },
        });
        let entries = compile_str(
            tempdir.path(),
            r#"
            path/to/target:
                ~source: path/to/source
                ~ignore: 
                  - ignore*
                  - ignore_dir/*
            "#,
        )
        .unwrap();
        let expected = touched_file_entries(tempdir.path(), AttrType::Copy);
        assert_eq!(entries, expected);
    }

    #[test]
    fn child_profile() {
        let tempdir = tempfile::tempdir().unwrap();
        create_tree!(tempdir.path(), {
            path: {
                to: {
                    source: {
                        dir1: {
                            file1,
                            file2,
                        },
                        dir2: {
                            file3,
                            file4,
                        },
                    },
                },
            },
        });
        let entries = compile_str(
            tempdir.path(),
            r#"
            path/to/target:
              ~source: path/to/source
              ~type: link
              ~recursive: true
              dir2:
                ~recursive: false
            "#,
        )
        .unwrap();
        let expected = compiled_entries(
            tempdir.path(),
            &["dir1/file1", "dir1/file2", "dir2"],
            AttrType::Link,
        );
        assert_eq!(entries, expected);
    }
}
