use super::ActionState;
use crate::application::task_manager::{
    actions::{IsAction, OutputExt},
    user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result, bail};
use common::utils;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    fs::OpenOptions,
    io::Write,
    process::{Command, Output},
    sync::Arc,
};
use tracing::{debug, error, info};

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
    fn run_action(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
        needs_user_context: bool,
    ) -> Result<()> {
        let mut command = self.get_command(user_context);
        if needs_user_context {
            user_context.apply_to(&mut command);
        }

        let command_output = command.output().context(match self {
            Self::Add { .. } => "Failed to run user group add action",
            Self::Remove { .. } => "Failed to run user group remove action",
        })?;

        output.append_output(command_output);

        Ok(())
    }

    fn needs_elevation(&self) -> bool {
        true
    }

    fn get_status(&self, user_context: &Arc<UserExecutionContext>) -> ActionState {
        let group = match self {
            Self::Add { group, .. } | Self::Remove { group, .. } => group,
        };
        let user = &user_context.user_name;

        debug!(action = %self, "Running status command");

        if !utils::command::test_command_available_sync("id") {
            let message = "'id' Command not available";
            error!(message);
            return ActionState::UnAvailable(message.to_string());
        }
        if !utils::command::test_command_available_sync("getent") {
            let message = "'getent' Command not available";
            error!(message);
            return ActionState::UnAvailable(message.to_string());
        }

        let Ok(output) = Command::new("id").arg("--groups").arg(user).output() else {
            let message = "Failed to run 'id --groups'";
            error!(message);
            return ActionState::UnAvailable(message.to_string());
        };

        if !output.status.success() {
            let stderr = utils::command::parse_output(&output.stderr);
            let message = "Failed to determine user group membership";
            error!(%user, %stderr, message);
            return ActionState::UnAvailable(message.to_string());
        }

        let groups = utils::command::parse_output(&output.stdout);

        let Ok(group_output) = Command::new("getent").arg("group").arg(group).output() else {
            let message = "Failed to run 'getent group'";
            error!(message);
            return ActionState::UnAvailable(message.to_string());
        };

        let group_exists = group_output.status.success();

        let is_member = if group_exists {
            let group_entry = utils::command::parse_output(&group_output.stdout);

            let Some(group_gid) = group_entry.split(':').nth(2) else {
                let message = "Failed to parse getent output for group {group}";
                error!(message);
                return ActionState::UnAvailable(message.to_string());
            };

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

        action_state
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

    fn before_retry(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
        needs_user_context: bool,
    ) -> Option<Result<()>> {
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

        debug!(
            group = %group,
            "Group might not exist; trying to create group before retry"
        );

        Some(Self::try_to_add_to_group(
            output,
            user_context,
            needs_user_context,
            group,
        ))
    }

    fn needs_reboot(&self) -> bool {
        true
    }
}
impl UserGroupAction {
    fn get_command(&self, user_context: &Arc<UserExecutionContext>) -> Command {
        let user = &user_context.user_name;

        let mut command = Command::new("gpasswd");

        match self {
            Self::Add { group, .. } => {
                command.arg("--add").arg(user).arg(group);
            }
            Self::Remove { group, .. } => {
                command.arg("--delete").arg(user).arg(group);
            }
        }

        command
    }

    #[allow(clippy::too_many_lines)]
    fn try_to_add_to_group(
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
        needs_user_context: bool,
        group: &String,
    ) -> Result<()> {
        debug!(group = %group, "Trying groupadd");

        let mut groupadd_command = Command::new("groupadd");
        groupadd_command.arg(group);
        if needs_user_context {
            user_context.apply_to(&mut groupadd_command);
        }

        match groupadd_command.status() {
            Ok(status) => {
                if status.success() {
                    let message = "Created group with groupadd";
                    info!(%group, message);
                    output.append_stdout(message);
                    return Ok(());
                }
            }
            Err(error) => {
                error!(group = %group, error = %error, "Failed to run groupadd");
                let message = "Failed to run groupadd";
                output.append_stderr(message);
                bail!(message)
            }
        }

        debug!(group = %group, "Trying manual add to system config file (group)");

        let mut getent_command = Command::new("getent");
        getent_command.arg("group");
        getent_command.arg(group);
        if needs_user_context {
            user_context.apply_to(&mut getent_command);
        }

        let group: Option<String> = match getent_command.output() {
            Ok(output) => {
                if output.status.success() {
                    Some(
                        utils::command::parse_output(&output.stdout)
                            .trim()
                            .to_string(),
                    )
                } else {
                    None
                }
            }
            Err(error) => {
                error!(group = %group, error = %error, "Failed to run getent");
                let message = "Failed to run getent";
                output.append_stderr(message);
                bail!(message);
            }
        };

        if let Some(group) = &group {
            debug!(group = %group, "Group exists; checking further");

            let system_config_group_file_path = &user_context.system_config_dir.join("group");

            let mut grep_command = Command::new("grep");
            grep_command.arg(group);
            grep_command.arg(system_config_group_file_path);
            if needs_user_context {
                user_context.apply_to(&mut getent_command);
            }

            let exists_in_system_config = match grep_command.status() {
                Ok(status) => {
                    if status.success() {
                        debug!(%group, ?system_config_group_file_path, "Group exists in system config file (group)");
                    } else {
                        debug!(%group, ?system_config_group_file_path, "Group does not exists in system config file (group)");
                    }

                    status.success()
                }
                Err(error) => {
                    error!(
                        %group,
                        ?system_config_group_file_path,
                        %error,
                        "Failed to check whether group exists in system config file (group)"
                    );

                    let message = "Failed to run getent";
                    output.append_stderr(message);
                    bail!(message)
                }
            };

            if !exists_in_system_config {
                debug!(%group, ?system_config_group_file_path, "Manually adding group to the system config file (group)");

                let file = OpenOptions::new()
                    .create(false)
                    .append(true)
                    .open(system_config_group_file_path)
                    .context("Failed to open the system config file")
                    .inspect_err(|error| error!(%error))?;

                match writeln!(&file, "{group}") {
                    Ok(()) => {
                        let message = "Wrote group manually in system config file";
                        info!(%group, ?file, message);
                        output.append_stdout(message);
                    }
                    Err(error) => {
                        let message = "Failed to write to file";
                        output.append_stderr(message);
                        error!(%group, file = ?system_config_group_file_path, %error, "Failed to write group manually in system config file");
                        bail!(message)
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use common::{
        app_dirs::AppDirs,
        cli::{Cli, Parser},
        config,
    };
    use std::rc::Rc;

    fn get_user_context() -> Arc<UserExecutionContext> {
        config::init();

        let arguments = Cli::parse_from::<[_; 0], &str>([]);
        let app_dirs = Rc::new(AppDirs::new(&arguments).unwrap());

        UserExecutionContext::new(arguments, &app_dirs).unwrap()
    }

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
        let user_context = get_user_context();

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
            vec!["--add", &utils::env::get_user_name(), "developers"]
        );
    }

    #[test]
    fn remove_command() {
        assert_eq!(
            command_args(&remove_action()),
            vec!["--delete", &utils::env::get_user_name(), "developers"]
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
        let user_context = get_user_context();

        let command = add_action().to_undo().get_command(&user_context);

        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["--delete", &utils::env::get_user_name(), "developers"]
        );
    }

    #[test]
    fn remove_undo_command_is_add() {
        let user_context = get_user_context();

        let command = remove_action().to_undo().get_command(&user_context);

        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["--add", &utils::env::get_user_name(), "developers"]
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
