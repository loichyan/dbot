use crate::error;
use serde::Serialize;
use std::path::Path;
use tera::{Context, Tera};
use thisctx::WithContext;

#[derive(Default)]
pub struct Renderer {
    tera: Tera,
    context: Context,
}

impl Renderer {
    pub fn new() -> Self {
        <_>::default()
    }

    pub fn add_data<T>(&mut self, key: impl Into<String>, val: &T)
    where
        T: ?Sized + Serialize,
    {
        self.context.insert(key, val);
    }

    pub fn render(&mut self, path: &Path) -> error::Result<String> {
        let content = std::fs::read_to_string(path).context(error::IoFailedContext { path })?;
        self.tera
            .render_str(&content, &self.context)
            .context(error::InvalidTemplateContext { path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn render_template() {
        let mut render = Renderer::default();
        render.add_data(
            "data",
            &serde_yaml::from_str::<serde_yaml::Value>(
                r#"
                key1: Hello,
                key2: DBot!
                "#,
            )
            .unwrap(),
        );
        let mut tempfile = NamedTempFile::new().unwrap();
        tempfile
            .write_all("{{ data.key1 }} {{ data.key2 }}".as_bytes())
            .unwrap();
        assert_eq!(render.render(tempfile.path()).unwrap(), "Hello, DBot!");
    }
}
