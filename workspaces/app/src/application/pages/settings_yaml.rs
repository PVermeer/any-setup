use super::YamlPage;
use crate::application::task_manager::{
    action_runner::ActionRunner,
    actions::{Action, ActionState, IsAction},
    user_execution_context::UserExecutionContext,
};
use anyhow::Result;
use gtk::InputPurpose;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    FreeForm,
    Digits,
    Number,
    Phone,
    Url,
    Email,
    Name,
    Password,
    Pin,
}
impl InputType {
    pub fn to_gtk(&self) -> InputPurpose {
        match self {
            Self::FreeForm => InputPurpose::FreeForm,
            Self::Digits => InputPurpose::Digits,
            Self::Number => InputPurpose::Number,
            Self::Phone => InputPurpose::Phone,
            Self::Url => InputPurpose::Url,
            Self::Email => InputPurpose::Email,
            Self::Name => InputPurpose::Name,
            Self::Password => InputPurpose::Password,
            Self::Pin => InputPurpose::Pin,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Input {
    pub title: String,
    pub input_type: InputType,
}

#[derive(Deserialize, Debug)]
pub struct Switch {
    pub title: String,
    pub subtitle: Option<String>,
    pub actions: Vec<Action>,
}
impl Switch {
    pub fn get_status(&self) -> ActionState {
        let status: Vec<ActionState> = self
            .actions
            .iter()
            .map(|action| action.get_status().unwrap_or_default())
            .collect();

        let done = status
            .iter()
            .all(|status| matches!(status, ActionState::Done));
        let available = status
            .iter()
            .all(|status| matches!(status, ActionState::UnAvailable));

        if done {
            return ActionState::Done;
        }
        if available {
            return ActionState::Available;
        }
        ActionState::UnAvailable
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Setting {
    Input(Input),
    Switch(Switch),
}

#[derive(Deserialize, Debug)]
pub struct Group {
    pub title: Option<String>,
    pub settings: Vec<Setting>,
}

#[derive(Deserialize, Debug)]
pub struct SettingsPageYaml {
    pub title: String,
    pub section: Option<String>,
    pub icon: String,
    pub groups: Vec<Group>,
}
impl YamlPage for SettingsPageYaml {
    fn get_action_runners(
        &self,
        user_context: &UserExecutionContext,
    ) -> Result<HashMap<u64, Arc<ActionRunner>>> {
        let mut map = HashMap::new();
        for group in &self.groups {
            for setting in &group.settings {
                match setting {
                    Setting::Input(_input) => {
                        // TODO
                    }
                    Setting::Switch(switch) => {
                        let action_runner =
                            ActionRunner::new(&switch.title, &switch.actions, user_context);
                        let action_runner_undo = action_runner.to_undo();

                        map.insert(action_runner.get_id(), action_runner);
                        map.insert(action_runner_undo.get_id(), action_runner_undo);
                    }
                }
            }
        }

        Ok(map)
    }
}
