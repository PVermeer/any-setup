mod dconf_value;

use super::{ActionState, OutputExt};
use crate::application::task_manager::{
    actions::IsAction, user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result, bail};
use common::utils;
use dconf_value::DconfValue;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    process::{Command, Output},
    sync::Arc,
};
use tracing::{debug, error};

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DconfAction {
    Set {
        schema: String,
        key: String,
        value: DconfValue,
    },

    #[serde(skip)]
    Reset { schema: String, key: String },
}
impl Display for DconfAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Set { schema, key, value } => write!(f, "Dconf set {schema} {key} to {value}"),
            Self::Reset { schema, key } => {
                write!(f, "Dconf reset {schema} {key}")
            }
        }
    }
}
impl IsAction for DconfAction {
    fn run_action(
        &self,
        output: &mut Output,
        _user_context: &Arc<UserExecutionContext>,
        _needs_user_context: bool,
    ) -> Result<()> {
        debug!(%self, "Running Dconf action");

        let mut command = self.get_command();
        let command_output = command
            .output()
            .context("Failed to run gsettings command")?;

        let success = command_output.status.success();

        if !success {
            error!(
                status = %command_output.status,
                stdout = utils::command::parse_output(&command_output.stdout),
                stderr = utils::command::parse_output(&command_output.stderr),
                "GSettings action failed"
            );
        }

        output.append_output(command_output);

        if !success {
            bail!("GSettings action failed");
        }

        Ok(())
    }

    fn needs_elevation(&self) -> bool {
        false
    }

    fn get_status(&self, _user_context: &Arc<UserExecutionContext>) -> ActionState {
        debug!(action = %self, "Running GSettings status command");

        if !utils::command::test_command_available_sync("gsettings") {
            error!("'gsettings' Command not available");
            return ActionState::UnAvailable;
        }

        let mut command = self.get_check_command();
        let Ok(command_output) = command.output() else {
            error!("Failed to run 'gsettings get'");
            return ActionState::UnAvailable;
        };

        if !command_output.status.success() {
            let stderr = utils::command::parse_output(&command_output.stderr);

            let message = format!("Failed to read GSettings value: {}", stderr.trim());

            error!(action = %self, %message);
            return ActionState::UnAvailable;
        }

        let current_value = utils::command::parse_output(&command_output.stdout);
        let current_value = current_value.trim();

        let action_state = match self {
            Self::Set { value, .. } => {
                if current_value == value.as_str() {
                    ActionState::Done
                } else {
                    ActionState::Available
                }
            }
            Self::Reset { .. } => ActionState::Available,
        };

        debug!(
            action = %self,
            current_value,
            action_state = %action_state,
            "GSettings action state"
        );

        action_state
    }

    fn fail_allowed(&self) -> bool {
        false
    }

    fn to_undo(&self) -> Self {
        match self {
            Self::Set { schema, key, .. } | Self::Reset { schema, key } => Self::Reset {
                schema: schema.clone(),
                key: key.clone(),
            },
        }
    }

    fn before_retry(
        &self,
        _output: &mut Output,
        _user_context: &Arc<UserExecutionContext>,
        _needs_user_context: bool,
    ) -> Option<Result<()>> {
        None
    }

    fn needs_reboot(&self) -> bool {
        false
    }
}
impl DconfAction {
    fn get_command(&self) -> Command {
        let mut command = Command::new("gsettings");

        match self {
            Self::Set { schema, key, value } => {
                debug!(
                    value = %value.as_str(),
                    "GSettings value argument"
                );

                command.arg("set").arg(schema).arg(key).arg(value.as_str());
            }
            Self::Reset { schema, key } => {
                command.arg("reset").arg(schema).arg(key);
            }
        }

        command
    }

    fn get_check_command(&self) -> Command {
        let mut command = Command::new("gsettings");

        match self {
            Self::Set { schema, key, .. } | Self::Reset { schema, key } => {
                command.arg("get").arg(schema).arg(key);
            }
        }

        command
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn set_action() -> DconfAction {
        DconfAction::Set {
            schema: "org.gnome.SessionManager".to_string(),
            key: "logout-prompt".to_string(),
            value: serde_yaml::from_str("\"false\"").unwrap(),
        }
    }

    fn reset_action() -> DconfAction {
        DconfAction::Reset {
            schema: "org.gnome.SessionManager".to_string(),
            key: "logout-prompt".to_string(),
        }
    }

    fn command_args(action: &DconfAction) -> Vec<String> {
        action
            .get_command()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn check_command_args(action: &DconfAction) -> Vec<String> {
        action
            .get_check_command()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn serde_yaml_parses_set_action() {
        let yaml = r#"
        type: set
        schema: org.gnome.SessionManager
        key: logout-prompt
        value: "false"
        "#;

        let action: DconfAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(action, set_action());
        assert_eq!(
            command_args(&action),
            vec!["set", "org.gnome.SessionManager", "logout-prompt", "false",]
        );
    }

    #[test]
    fn serde_yaml_round_trips_set_action() {
        let action = set_action();

        let yaml = serde_yaml::to_string(&action).unwrap();
        let parsed: DconfAction = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed, action);
    }

    #[test]
    fn serde_yaml_rejects_internal_reset_action() {
        let yaml = r"
        type: reset
        schema: org.gnome.SessionManager
        key: logout-prompt
        ";

        assert!(serde_yaml::from_str::<DconfAction>(yaml).is_err());
    }

    #[test]
    fn set_command_uses_gsettings_arguments() {
        let action = set_action();

        assert_eq!(
            command_args(&action),
            vec!["set", "org.gnome.SessionManager", "logout-prompt", "false",]
        );
    }

    #[test]
    fn reset_command_uses_gsettings_arguments() {
        let action = reset_action();

        assert_eq!(
            command_args(&action),
            vec!["reset", "org.gnome.SessionManager", "logout-prompt",]
        );
    }

    #[test]
    fn check_command_uses_gsettings_get() {
        let action = set_action();

        assert_eq!(
            check_command_args(&action),
            vec!["get", "org.gnome.SessionManager", "logout-prompt",]
        );
    }

    #[test]
    fn reset_check_command_uses_gsettings_get() {
        let action = reset_action();

        assert_eq!(
            check_command_args(&action),
            vec!["get", "org.gnome.SessionManager", "logout-prompt",]
        );
    }

    #[test]
    fn set_undo_is_reset() {
        let action = set_action();

        assert_eq!(action.to_undo(), reset_action());
    }

    #[test]
    fn reset_undo_is_reset() {
        let action = reset_action();

        assert_eq!(action.to_undo(), reset_action());
    }

    #[test]
    fn display_set_action() {
        assert_eq!(
            set_action().to_string(),
            "Dconf set org.gnome.SessionManager logout-prompt to false"
        );
    }

    #[test]
    fn display_reset_action() {
        assert_eq!(
            reset_action().to_string(),
            "Dconf reset org.gnome.SessionManager logout-prompt"
        );
    }

    #[test]
    fn action_flags_are_correct() {
        let action = set_action();

        assert!(!action.needs_elevation());
        assert!(!action.fail_allowed());
        assert!(!action.needs_reboot());
    }

    #[test]
    fn reset_action_flags_are_correct() {
        let action = reset_action();

        assert!(!action.needs_elevation());
        assert!(!action.fail_allowed());
        assert!(!action.needs_reboot());
    }
}
