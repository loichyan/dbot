use crate::{error, Runtime, F_PROFILE};
use dbot::Merge;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thisctx::WithContext;

#[derive(Deserialize)]
pub struct Profile {
    import: Vec<PathBuf>,
    #[serde(flatten)]
    pub content: ProfileContent,
}

#[derive(Deserialize)]
pub struct ProfileContent {
    pub data: Option<serde_yaml::Mapping>,
    pub profile: Option<dbot::Profile>,
}

impl Merge for ProfileContent {
    fn merge(&mut self, other: Self) {
        self.data.merge(other.data);
        self.profile.merge(other.profile);
    }
}

impl Runtime {
    pub fn load_profile(&self, source: &Path) -> error::Result<Profile> {
        let path = source.join(F_PROFILE);
        let content = std::fs::read_to_string(&path).context(error::Io(&path))?;
        let mut new = serde_yaml::from_str::<Profile>(&content).context(error::Yaml(&path))?;
        for file in new.import.iter() {
            let content = std::fs::read_to_string(source.join(file)).context(error::Io(&file))?;
            let import =
                serde_yaml::from_str::<ProfileContent>(&content).context(error::Yaml(&file))?;
            new.content.merge(import);
        }
        Ok(new)
    }
}
