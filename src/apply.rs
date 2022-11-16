use crate::{compile::CompiledEntries, error, profile::AttrType, template::Renderer};
use dotfish::DotFish;
use std::path::Path;
use thisctx::WithContext;

fn create_symlink(original: &Path, link: &Path) -> error::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, link)
            .context(error::IoFailedContext { path: original })?;
    }
    #[cfg(not(unix))]
    {
        unreachable!("symlinks are only supported on unix systems")
    }

    Ok(())
}

pub fn apply(renderer: &mut Renderer, entries: &CompiledEntries) -> error::Result<()> {
    for (target, profile) in entries.iter() {
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).context(error::IoFailedContext { path: dir })?;
        }
        match profile.ty {
            AttrType::Template => {
                let content = renderer.render(&profile.source)?;
                std::fs::write(target, content).context(error::IoFailedContext {
                    path: &profile.source,
                })?;
            }
            AttrType::Copy => std::fs::copy(&profile.source, target)
                .context(error::IoFailedContext {
                    path: &profile.source,
                })?
                .ignore2(),
            AttrType::Link => create_symlink(&profile.source, target)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile, compile::CompilerOptions};
    use std::path::{Path, PathBuf};

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

    fn apply_profile(profile: &str, source: PathBuf, target: PathBuf) {
        let entries = compile(
            &CompilerOptions { source, target },
            serde_yaml::from_str(profile).unwrap(),
        )
        .unwrap();
        apply(&mut <_>::default(), &entries).unwrap();
    }

    #[test]
    fn apply_copy() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        create_tmp_tree(source.path());
        apply_profile(
            r#"
            path/to/target: path/to/source
            "#,
            source.path().into(),
            target.path().into(),
        );
        test_tree!(target.path(), {
            path: {
                to: {
                    target: {
                        file1: (is_file, is_file),
                        file2: (is_file, is_file),
                    },
                },
            },
        });
    }

    #[test]
    fn apply_template() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        create_tmp_tree(source.path());
        apply_profile(
            r#"
            path/to/target: 
              ~source: path/to/source
              ~type: template
              ~recursive: true
            "#,
            source.path().into(),
            target.path().into(),
        );
        test_tree!(target.path(), {
            path: {
                to: {
                    target: {
                        file1: (is_file, is_file),
                        file2: (is_file, is_file),
                    },
                },
            },
        });
    }

    #[cfg(unix)]
    #[test]
    fn apply_link_file() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        create_tmp_tree(source.path());
        apply_profile(
            r#"
            path/to/target:
              ~source: path/to/source
              ~type: link
              ~recursive: true
            "#,
            source.path().into(),
            target.path().into(),
        );
        test_tree!(target.path(), {
            path: {
                to: {
                    target: {
                        file1: (is_file, is_symlink),
                        file2: (is_file, is_symlink),
                    },
                },
            },
        });
    }

    #[cfg(unix)]
    #[test]
    fn apply_link_dir() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        create_tmp_tree(source.path());
        apply_profile(
            r#"
            path/to/target:
              ~source: path/to/source
              ~type: link
            "#,
            source.path().into(),
            target.path().into(),
        );
        test_tree!(target.path(), {
            path: {
                to: {
                    target: (is_dir, is_symlink),
                },
            },
        });
    }
}
