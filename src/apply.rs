use crate::{compile::CompiledEntries, error, profile::AttrType};
use std::path::Path;
use thisctx::WithContext;

pub trait TemplateRenderer {
    type Err;

    fn render(&mut self, s: &str) -> Result<String, Self::Err>;
}

fn create_symlink(original: &Path, link: &Path) -> error::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, link).context(error::IoFailed(original))?;
    }
    #[cfg(not(unix))]
    {
        unreachable!("symlinks are only supported on unix systems")
    }

    Ok(())
}

pub fn apply<R: TemplateRenderer>(renderer: &mut R, entries: &CompiledEntries) -> error::Result<()>
where
    R::Err: 'static + std::error::Error + Send + Sync,
{
    for (target, profile) in entries.0.iter() {
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).context(error::IoFailed(dir))?;
        }
        match profile.ty {
            AttrType::Template => {
                let path = &profile.source;
                let s = std::fs::read_to_string(path).context(error::IoFailed(path))?;
                let content = renderer
                    .render(&s)
                    .map_err(|e| Box::new(e) as Box<_>)
                    .context(error::RenderError(path))?;
                std::fs::write(target, content).context(error::IoFailed(path))?;
            }
            AttrType::Copy => {
                std::fs::copy(&profile.source, target).context(error::IoFailed(&profile.source))?;
            }
            AttrType::Link => create_symlink(&profile.source, target)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile, compile::CompilerOptions, Profile};
    use std::path::Path;
    use thiserror::Error;

    struct EmptyRenderer;

    #[derive(Debug, Error)]
    #[error("")]
    struct RendererErr;

    impl TemplateRenderer for EmptyRenderer {
        type Err = RendererErr;

        fn render(&mut self, s: &str) -> Result<String, Self::Err> {
            Ok(s.to_owned())
        }
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

    fn apply_profile(profile: &str, source: &Path, target: &Path) {
        let entries = compile(
            &CompilerOptions { source, target },
            serde_yaml::from_str::<Profile>(profile)
                .unwrap()
                .into_entries()
                .unwrap(),
        )
        .unwrap();
        apply(&mut EmptyRenderer, &entries).unwrap();
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
            source.path(),
            target.path(),
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
            source.path(),
            target.path(),
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
            source.path(),
            target.path(),
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
            source.path(),
            target.path(),
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
