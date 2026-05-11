use super::{DynPage, Page, content::ContentPage, settings::SettingsPage};
use crate::application::task_manager::TaskManager;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::PathBuf, rc::Rc};

#[derive(PartialEq, Deserialize, Debug)]
#[serde(tag = "page_type", rename_all = "lowercase")]
pub enum PageYaml {
    Content(ContentPage),
    Settings(SettingsPage),
}
impl PageYaml {
    pub fn from_file(file_path: &PathBuf) -> Result<Self> {
        let file_string = fs::read_to_string(file_path).context(format!(
            "Failed to read file to string: {}",
            file_path.display()
        ))?;

        serde_yaml::from_str(&file_string)
            .context(format!("Not a valid page yaml: {}", file_path.display()))
    }

    pub fn into_page(self, action_manager: &Rc<TaskManager>) -> Page {
        match self {
            Self::Content(p) => p.build_page(action_manager),
            Self::Settings(p) => p.build_page(action_manager),
        }
    }
}
