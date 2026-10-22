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
use tracing::debug;
use url::Url;

#[derive(Deserialize, Clone, Debug)]
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

#[derive(Deserialize, Clone, Debug)]
pub struct Input {
    pub title: String,
    pub input_type: InputType,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Switch {
    pub title: String,
    pub subtitle: Option<String>,
    pub actions: Vec<Action>,
}

impl Switch {
    pub fn get_status(&self, user_context: &Arc<UserExecutionContext>) -> ActionState {
        let action_states: Vec<ActionState> = self
            .actions
            .iter()
            .map(|action| action.get_status(user_context))
            .collect();

        let final_action_state = if let Some(unavailable) = action_states
            .iter()
            .find(|state| matches!(state, ActionState::UnAvailable(_)))
        {
            unavailable.clone()
        } else if action_states
            .iter()
            .all(|state| matches!(state, ActionState::Done))
        {
            ActionState::Done
        } else if action_states
            .iter()
            .any(|state| matches!(state, ActionState::Available))
        {
            ActionState::Available
        } else {
            ActionState::UnAvailable("This should not happen".to_string())
        };

        debug!(?action_states, action_state = %final_action_state, "Action state for Switch");

        final_action_state
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Setting {
    Input(Input),
    Switch(Switch),
}

#[derive(Deserialize, Debug)]
pub struct Link {
    pub title: String,
    pub url: Url,
}

#[derive(Deserialize, Debug)]
pub struct Group {
    pub title: Option<String>,
    pub description: Option<String>,
    pub link: Option<Link>,
    pub settings: Vec<Setting>,
}

#[derive(Deserialize, Debug)]
pub struct SettingsPageYaml {
    pub title: String,
    pub section: Option<String>,
    pub icon: Option<String>,
    pub groups: Vec<Group>,
}

impl YamlPage for SettingsPageYaml {
    fn get_action_runners(
        &self,
        user_context: &Arc<UserExecutionContext>,
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
