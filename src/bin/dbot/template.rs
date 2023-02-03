use dbot::TemplateRenderer;
use serde::Serialize;
use tera::{Context, Tera};

#[derive(Default)]
pub struct TeraRenderer {
    tera: Tera,
    context: Context,
}

impl TemplateRenderer for TeraRenderer {
    type Err = tera::Error;

    fn render(&mut self, s: &str) -> Result<String, Self::Err> {
        self.tera.render_str(s, &self.context)
    }
}

impl TeraRenderer {
    pub fn add_data<T>(&mut self, key: impl Into<String>, val: &T)
    where
        T: ?Sized + Serialize,
    {
        self.context.insert(key, val);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, path::Path};
    use tempfile::NamedTempFile;

    fn render_path(renderer: &mut TeraRenderer, path: &Path) -> String {
        let content = std::fs::read_to_string(path).unwrap();
        renderer.render(&content).unwrap()
    }

    #[test]
    fn render_template() {
        let mut render = TeraRenderer::default();
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
        assert_eq!(render_path(&mut render, tempfile.path()), "Hello, DBot!");
    }
}
