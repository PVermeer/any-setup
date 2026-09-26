use super::{
    DynPage, YamlPage, content::ContentPage, content_yaml::ContentPageYaml, settings::SettingsPage,
    settings_yaml::SettingsPageYaml,
};
use crate::application::task_manager::{
    TaskManager, action_runner::ActionRunner, user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf, rc::Rc, sync::Arc};
use tracing::debug;

#[derive(Deserialize, Debug)]
#[serde(tag = "page_type", rename_all = "lowercase")]
pub enum PageYaml {
    Content(ContentPageYaml),
    Settings(SettingsPageYaml),
}
impl PageYaml {
    pub fn from_file(file_path: &PathBuf) -> Result<Self> {
        debug!(?file_path, "Reading yaml file");

        let file_string = fs::read_to_string(file_path).context(format!(
            "Failed to read file to string: {}",
            file_path.display()
        ))?;

        debug!(?file_path, "Parsing yaml file");

        serde_yaml::from_str(&file_string)
            .context(format!("Not a valid page yaml: {}", file_path.display()))
    }

    pub fn into_page(
        self,
        task_manager: &Rc<TaskManager>,
        user_context: &Arc<UserExecutionContext>,
    ) -> Result<Rc<dyn DynPage>> {
        match self {
            Self::Content(yaml) => ContentPage::new(yaml).build_page(task_manager, user_context),
            Self::Settings(yaml) => SettingsPage::new(yaml).build_page(task_manager, user_context),
        }
    }

    pub fn into_action_runners(
        self,
        user_context: &Arc<UserExecutionContext>,
    ) -> Result<HashMap<u64, Arc<ActionRunner>>> {
        match self {
            Self::Content(yaml) => yaml.get_action_runners(user_context),
            Self::Settings(yaml) => yaml.get_action_runners(user_context),
        }
    }
}
