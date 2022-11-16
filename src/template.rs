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
        let name = path.to_str().expect("path must be a valid UTF-8 string");
        let content = std::fs::read_to_string(path).context(error::IoFailedContext { path })?;
        self.tera
            .add_raw_template(name, &content)
            .context(error::InvalidTemplateContext { path })?;
        self.tera
            .render(name, &self.context)
            .context(error::InvalidTemplateContext { path })
    }
}

// TODO: add tests
