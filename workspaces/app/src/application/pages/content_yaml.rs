use super::YamlPage;
use crate::application::task_manager::{
    action_runner::ActionRunner, user_execution_context::UserExecutionContext,
};
use anyhow::Result;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Fill,
}

#[derive(Deserialize, Debug)]
pub struct Header {
    pub icon: Option<String>,
    pub text: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Content {
    #[serde(default)]
    pub pango: bool,

    #[serde(default)]
    pub align: TextAlign,

    pub text: String,
}

#[derive(Deserialize, Debug)]
pub struct ContentPageYaml {
    pub title: String,
    pub section: Option<String>,
    pub icon: String,
    pub header: Option<Header>,
    pub contents: Option<Vec<Content>>,
}
impl YamlPage for ContentPageYaml {
    fn get_action_runners(
        &self,
        _user_context: &UserExecutionContext,
    ) -> Result<HashMap<u64, Arc<ActionRunner>>> {
        Ok(HashMap::default())
    }
}
