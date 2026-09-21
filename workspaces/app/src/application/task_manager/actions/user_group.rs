use super::ActionState;
use crate::application::task_manager::{
    actions::IsAction, user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result, anyhow, bail};
use common::utils;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, process::Command};
use tracing::{debug, error};

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UserGroupAction {
    Add {
        group: String,
        create_if_missing: Option<bool>,
        fail_allowed: Option<bool>,
    },
    Remove {
        group: String,
        fail_allowed: Option<bool>,
    },
}
impl Display for UserGroupAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add { group, .. } => {
                write!(f, "Add user to group {group}")
            }
            Self::Remove { group, .. } => {
                write!(f, "Remove user from group {group}")
            }
        }
    }
}
impl IsAction for UserGroupAction {
    fn get_command(&self, user_context: &UserExecutionContext) -> Command {
        let user = &user_context.user_name;

        match self {
            Self::Add { group, .. } => {
                let mut command = Command::new("usermod");
                command.arg("--append").arg("--groups").arg(group).arg(user);

                command
            }

            Self::Remove { group, .. } => {
                let mut command = Command::new("gpasswd");
                command.arg("--delete").arg(user).arg(group);

                command
            }
        }
    }

    fn needs_elevation(&self) -> bool {
        true
    }

    fn get_status(&self, user_context: &UserExecutionContext) -> Result<ActionState> {
        let group = match self {
            Self::Add { group, .. } | Self::Remove { group, .. } => group,
        };
        let user = &user_context.user_name;

        debug!(action = %self, "Running status command");

        let output = Command::new("id")
            .arg("--groups")
            .arg(user)
            .output()
            .context("Failed to run id command")?;

        if !output.status.success() {
            let stderr = utils::command::parse_output(&output.stderr);

            error!(
                user = %user,
                stderr = %stderr,
                "Failed to determine user group membership"
            );

            bail!("Failed to determine groups for user {user}: {stderr}");
        }

        let groups = utils::command::parse_output(&output.stdout);

        let group_output = Command::new("getent")
            .arg("group")
            .arg(group)
            .output()
            .context("Failed to run getent command")?;

        let group_exists = group_output.status.success();

        let is_member = if group_exists {
            let group_entry = utils::command::parse_output(&group_output.stdout);

            let group_gid = group_entry
                .split(':')
                .nth(2)
                .ok_or_else(|| anyhow!("Invalid getent output for group {group}"))?;

            groups.split_whitespace().any(|gid| gid == group_gid)
        } else {
            false
        };

        let action_state = match self {
            Self::Add { .. } => {
                if is_member {
                    ActionState::Done
                } else {
                    ActionState::Available
                }
            }

            Self::Remove { .. } => {
                if is_member {
                    ActionState::Available
                } else {
                    ActionState::Done
                }
            }
        };

        debug!(
            action = %self,
            group = %group,
            group_exists,
            is_member,
            action_state = %action_state,
            "Action state"
        );

        Ok(action_state)
    }

    fn fail_allowed(&self) -> bool {
        match self {
            Self::Add { fail_allowed, .. } | Self::Remove { fail_allowed, .. } => {
                fail_allowed.is_some_and(|value| value)
            }
        }
    }

    fn to_undo(&self) -> Self {
        match self.clone() {
            Self::Add {
                group,
                fail_allowed,
                ..
            } => Self::Remove {
                group,
                fail_allowed,
            },

            Self::Remove {
                group,
                fail_allowed,
            } => Self::Add {
                group,
                create_if_missing: Some(false),
                fail_allowed,
            },
        }
    }

    fn on_error(&self, _output: &std::process::Output) -> Option<Command> {
        let Self::Add {
            group,
            create_if_missing,
            ..
        } = self
        else {
            return None;
        };

        if !create_if_missing.unwrap_or(true) {
            return None;
        }

        let group_exists = match Command::new("getent").arg("group").arg(group).output() {
            Ok(output) => output.status.success(),
            Err(error) => {
                error!(
                    group = %group,
                    error = %error,
                    "Failed to check whether group exists"
                );

                return None;
            }
        };

        if group_exists {
            debug!(
                group = %group,
                "Group already exists; no recovery action needed"
            );

            return None;
        }

        debug!(
            group = %group,
            "Group does not exist; creating group before retry"
        );

        let mut command = Command::new("groupadd");
        command.arg(group);

        Some(command)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn add_action() -> UserGroupAction {
        UserGroupAction::Add {
            group: "developers".to_string(),
            create_if_missing: Some(true),
            fail_allowed: Some(false),
        }
    }

    fn remove_action() -> UserGroupAction {
        UserGroupAction::Remove {
            group: "developers".to_string(),
            fail_allowed: Some(false),
        }
    }

    fn command_args(action: &UserGroupAction) -> Vec<String> {
        let user_context = UserExecutionContext::new().unwrap();

        action
            .get_command(&user_context)
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn add_command() {
        assert_eq!(
            command_args(&add_action()),
            vec![
                "--append",
                "--groups",
                "developers",
                &utils::env::get_user_name(),
            ]
        );
    }

    #[test]
    fn remove_command() {
        assert_eq!(
            command_args(&remove_action()),
            vec!["--delete", &utils::env::get_user_name(), "developers",]
        );
    }

    #[test]
    fn add_needs_elevation() {
        assert!(add_action().needs_elevation());
    }

    #[test]
    fn remove_needs_elevation() {
        assert!(remove_action().needs_elevation());
    }

    #[test]
    fn add_is_not_fail_allowed() {
        assert!(!add_action().fail_allowed());
    }

    #[test]
    fn remove_is_not_fail_allowed() {
        assert!(!remove_action().fail_allowed());
    }

    #[test]
    fn add_undoes_to_remove() {
        assert_eq!(
            add_action().to_undo(),
            UserGroupAction::Remove {
                group: "developers".to_string(),
                fail_allowed: Some(false),
            }
        );
    }

    #[test]
    fn remove_undoes_to_add_without_group_creation() {
        assert_eq!(
            remove_action().to_undo(),
            UserGroupAction::Add {
                group: "developers".to_string(),
                create_if_missing: Some(false),
                fail_allowed: Some(false),
            }
        );
    }

    #[test]
    fn add_undo_command_is_remove() {
        let user_context = UserExecutionContext::new().unwrap();

        let command = add_action().to_undo().get_command(&user_context);

        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["--delete", &utils::env::get_user_name(), "developers",]
        );
    }

    #[test]
    fn remove_undo_command_is_add() {
        let user_context = UserExecutionContext::new().unwrap();

        let command = remove_action().to_undo().get_command(&user_context);

        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec![
                "--append",
                "--groups",
                "developers",
                &utils::env::get_user_name(),
            ]
        );
    }

    #[test]
    fn serde_yaml_parses_add_action() {
        let yaml = r"
            type: add
            group: developers
            create_if_missing: true
            fail_allowed: false
        ";

        let action: UserGroupAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            UserGroupAction::Add {
                group: "developers".to_string(),
                create_if_missing: Some(true),
                fail_allowed: Some(false),
            }
        );
    }

    #[test]
    fn serde_yaml_parses_remove_action() {
        let yaml = r"
            type: remove
            group: developers
            fail_allowed: false
        ";

        let action: UserGroupAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            UserGroupAction::Remove {
                group: "developers".to_string(),
                fail_allowed: Some(false),
            }
        );
    }

    #[test]
    fn serde_yaml_round_trips_add_action() {
        let action = add_action();

        let yaml = serde_yaml::to_string(&action).unwrap();
        let parsed: UserGroupAction = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed, action);
    }

    #[test]
    fn serde_yaml_round_trips_remove_action() {
        let action = remove_action();

        let yaml = serde_yaml::to_string(&action).unwrap();
        let parsed: UserGroupAction = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed, action);
    }
}
