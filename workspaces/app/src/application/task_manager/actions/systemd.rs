use super::{Action, ActionState, OutputExt};
use crate::application::task_manager::{
    actions::IsAction, user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result, anyhow, bail};
use common::utils;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    fs,
    os::unix::process::ExitStatusExt,
    process::{Command, ExitStatus, Output},
    str::FromStr,
    sync::Arc,
};
use tracing::{debug, error, info, warn};

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    System,
    User,
}

impl Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
        }
    }
}

impl Scope {
    fn to_arg(&self) -> String {
        format!("--{self}")
    }
}

/// Enum for mapping systemctl is-enabled output.
enum IsEnabledOutput {
    /// Will start at boot (has proper symlinks)
    Enabled,
    /// Enabled, but only until next reboot
    EnabledRuntime,
    /// Not enabled
    Disabled,
    /// Not found
    NotFound,
    /// Completely blocked (cannot be started at all)
    Masked,
    /// Masked until next reboot
    MaskedRuntime,
    /// Has no [Install] section; can’t be enabled directly (only pulled in as a dependency)
    Static,
    /// Not enabled itself, but referenced by another unit’s install config
    Indirect,
    /// This name is just an alias of another unit
    Alias,
    /// Unit file is symlinked from outside standard directories
    Linked,
    /// Same as above, but temporary
    LinkedRuntime,
    /// Created dynamically by systemd generators at boot
    Generated,
    /// Created at runtime (e.g. via systemd-run)
    Transient,
    /// Invalid or broken unit file
    Bad,
}

impl Display for IsEnabledOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Enabled => write!(f, "enabled"),
            Self::EnabledRuntime => write!(f, "enabled-runtime"),
            Self::Disabled => write!(f, "disabled"),
            Self::NotFound => write!(f, "not-found"),
            Self::Masked => write!(f, "masked"),
            Self::MaskedRuntime => write!(f, "masked-runtime"),
            Self::Static => write!(f, "static"),
            Self::Indirect => write!(f, "indirect"),
            Self::Alias => write!(f, "alias"),
            Self::Linked => write!(f, "linked"),
            Self::LinkedRuntime => write!(f, "linked-runtime"),
            Self::Generated => write!(f, "generated"),
            Self::Transient => write!(f, "transient"),
            Self::Bad => write!(f, "bad"),
        }
    }
}

impl FromStr for IsEnabledOutput {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let output_string = s.trim();
        match output_string {
            "enabled" => Ok(Self::Enabled),
            "enabled-runtime" => Ok(Self::EnabledRuntime),
            "disabled" => Ok(Self::Disabled),
            "not-found" => Ok(Self::NotFound),
            "masked" => Ok(Self::Masked),
            "masked-runtime" => Ok(Self::MaskedRuntime),
            "static" => Ok(Self::Static),
            "indirect" => Ok(Self::Indirect),
            "alias" => Ok(Self::Alias),
            "linked" => Ok(Self::Linked),
            "linked-runtime" => Ok(Self::LinkedRuntime),
            "generated" => Ok(Self::Generated),
            "transient" => Ok(Self::Transient),
            "bad" => Ok(Self::Bad),
            _ => {
                error!(
                    output = output_string,
                    "Failed to match 'systemctl is-enabled' output"
                );
                Err(anyhow!("Failed to match 'systemctl is-enabled' output"))
            }
        }
    }
}

impl IsEnabledOutput {
    fn from_action(
        action: &SystemdAction,
        user_context: Option<&Arc<UserExecutionContext>>,
    ) -> Result<Self> {
        let mut command = action.get_check_command();

        if let Some(user_context) = user_context {
            user_context.apply_to(&mut command);
        }

        let output = command.output().context("Failed to run check command")?;
        let stdout = utils::command::parse_output(&output.stdout);

        Self::from_str(&stdout)
    }

    fn to_action_state(
        &self,
        action: &SystemdAction,
        user_context: Option<&Arc<UserExecutionContext>>,
        recursive_depth: Option<u32>,
    ) -> ActionState {
        fn handle_alias(
            action: &SystemdAction,
            user_context: Option<&Arc<UserExecutionContext>>,
            recursive_depth: Option<u32>,
        ) -> ActionState {
            debug!("Found alias");

            let depth = recursive_depth.unwrap_or(0);
            if depth > 10 {
                let message = "Reached max recursive depth trying to resolve an alias";
                error!(
                    %action,
                    depth,
                    message
                );
                return ActionState::UnAvailable(message.to_string());
            }

            let Ok(resolved_unit_name) = action.resolve_alias() else {
                let message = "Failed to resolve alias for";
                error!(message);
                return ActionState::UnAvailable(message.to_string());
            };
            let mut action_clone = action.clone();
            match &mut action_clone {
                SystemdAction::Enable { unit, .. } | SystemdAction::Disable { unit, .. } => {
                    *unit = resolved_unit_name;
                }
            }
            let Ok(new_is_enabled_output) =
                IsEnabledOutput::from_action(&action_clone, user_context)
            else {
                let message = "Unable to resolve unit alias";
                warn!("Unable to resolve unit alias");
                return ActionState::UnAvailable(message.to_string());
            };

            debug!(
                %new_is_enabled_output,
                "Re-running alias from depth: {depth}"
            );

            new_is_enabled_output.to_action_state(&action_clone, user_context, Some(depth + 1))
        }

        match action {
            SystemdAction::Enable { unit, .. } => match self {
                Self::Enabled => ActionState::Done,
                Self::Disabled => ActionState::Available,
                Self::Alias => handle_alias(action, user_context, recursive_depth),
                _ => ActionState::UnAvailable(format!("Unable to handle unit {unit}: {self}")),
            },

            SystemdAction::Disable { unit, .. } => match self {
                Self::Enabled => ActionState::Available,
                Self::Disabled => ActionState::Done,
                Self::Alias => handle_alias(action, user_context, recursive_depth),
                _ => ActionState::UnAvailable(format!("Unable to handle unit {unit}: {self}")),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SystemdAction {
    Enable {
        unit: String,
        scope: Scope,
        now: Option<bool>,
        fail_allowed: Option<bool>,
    },
    Disable {
        unit: String,
        scope: Scope,
        now: Option<bool>,
        fail_allowed: Option<bool>,
    },
}

impl Display for SystemdAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Enable {
                unit: name, scope, ..
            } => write!(f, "Systemd enable {scope} unit: {name}"),

            Self::Disable {
                unit: name, scope, ..
            } => write!(f, "Systemd disable {scope} unit: {name}"),
        }
    }
}

impl SystemdAction {
    fn get_command(&self) -> Command {
        match self {
            Self::Enable {
                unit, scope, now, ..
            } => {
                let mut command = Command::new("systemctl");
                command.arg(scope.to_arg());
                command.arg("enable");

                if now.is_none_or(|now| now) {
                    command.arg("--now");
                }

                command.arg(unit);

                command
            }

            Self::Disable {
                unit, scope, now, ..
            } => {
                let mut command = Command::new("systemctl");
                command.arg(scope.to_arg());
                command.arg("disable");

                if now.is_none_or(|now| now) {
                    command.arg("--now");
                }

                command.arg(unit);

                command
            }
        }
    }

    fn get_check_command(&self) -> Command {
        match self {
            Self::Enable { unit, scope, .. } | Self::Disable { unit, scope, .. } => {
                let mut command = Command::new("systemctl");
                command.arg(scope.to_arg()).arg("is-enabled").arg(unit);

                command
            }
        }
    }

    fn resolve_alias(&self) -> Result<String> {
        debug!("Resolving alias");

        match self {
            Self::Enable { unit, scope, .. } | Self::Disable { unit, scope, .. } => {
                let mut command = Command::new("systemctl");

                command
                    .arg(scope.to_arg())
                    .arg("show")
                    .arg("--property=Id")
                    .arg("--value")
                    .arg(unit);

                let output = command
                    .output()
                    .context("Failed to run systemd alias resolve command")?;

                if !output.status.success() {
                    let message = format!("Failed to resolve systemd unit alias for: {unit}");
                    error!(message);
                    bail!(message);
                }

                let output = utils::command::parse_output(&output.stdout);
                let output = output.trim().to_string();

                info!(output, "Resolved alias");

                Ok(output)
            }
        }
    }

    fn is_enabled_for_other_users(&self, user_context: &Arc<UserExecutionContext>) -> bool {
        match self {
            Self::Disable { unit, scope, .. } => {
                if matches!(scope, Scope::System) {
                    return false;
                }

                let user_home_dir_name = user_context.user_home_dir.file_name().unwrap_or_default();

                debug!(
                    unit,
                    user_home_dir_name = %user_home_dir_name.to_string_lossy(),
                    "Checking if other users are using services"
                );

                let Ok(users) = fs::read_dir("/home") else {
                    return false;
                };

                for user in users.flatten() {
                    if user.file_name().as_os_str() == user_home_dir_name {
                        continue;
                    }

                    let config = user.path().join(".config/systemd/user");

                    let Ok(targets) = fs::read_dir(config) else {
                        continue;
                    };

                    for target in targets.flatten() {
                        let name = target.file_name();
                        let name = name.to_string_lossy();

                        if !name.ends_with(".target.wants") && !name.ends_with(".target.requires") {
                            continue;
                        }

                        if target.path().join(unit).is_symlink() {
                            debug!(
                                %unit,
                                other_user_dir_name = %&user.file_name().to_string_lossy(),
                                "Found other user that uses service"
                            );

                            return true;
                        }
                    }
                }

                debug!(unit, "No other users are using service");

                false
            }

            Self::Enable { .. } => false,
        }
    }
}

impl IsAction for SystemdAction {
    fn run_action(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
        needs_user_context: bool,
        actions: &[Action],
    ) -> Result<()> {
        // Don't disable the system unit if other users have the user units enabled
        if let Self::Disable { scope, .. } = self
            && matches!(scope, Scope::System)
            && actions.len() > 1
        {
            for action in actions {
                if let Action::SystemD(systemd_action) = action
                    && let Self::Disable { .. } = systemd_action
                    && systemd_action.is_enabled_for_other_users(user_context)
                {
                    output.append_stdout("Cancel action");
                    output.append_stdout("Other users are still using this unit");
                    output.status = ExitStatus::from_raw(0);

                    return Ok(());
                }
            }
        }

        let mut command = self.get_command();

        if needs_user_context {
            user_context.apply_to(&mut command);
        }

        let command_output = command.output().context(match self {
            Self::Enable { .. } => "Failed to run systemd enable action",
            Self::Disable { .. } => "Failed to run systemd disable action",
        })?;

        output.append_output(command_output);

        Ok(())
    }

    fn needs_elevation(&self) -> bool {
        match self {
            Self::Enable { scope, .. } | Self::Disable { scope, .. } => {
                matches!(scope, Scope::System)
            }
        }
    }

    fn get_status(
        &self,
        user_context: &Arc<UserExecutionContext>,
        needs_user_context: bool,
    ) -> ActionState {
        debug!(action = %self, "Running status command");

        let user_context = needs_user_context.then_some(user_context);

        if !utils::command::test_command_available_sync("systemctl") {
            let message = "'systemctl' Command not available";
            error!(message);
            return ActionState::UnAvailable(message.to_string());
        }

        let Ok(is_enabled_output) = IsEnabledOutput::from_action(self, user_context) else {
            let message = "Failed to get is-enabled output";
            error!(message);
            return ActionState::UnAvailable(message.to_string());
        };
        let action_state = is_enabled_output.to_action_state(self, user_context, None);

        debug!(
            action = %self,
            is_enabled_output = %is_enabled_output,
            action_state = %action_state,
            "Action state"
        );

        action_state
    }

    fn fail_allowed(&self) -> bool {
        match self {
            Self::Enable { fail_allowed, .. } | Self::Disable { fail_allowed, .. } => {
                let allowed_to_fail = fail_allowed.is_some_and(|fail_allowed| fail_allowed);

                debug!(allowed_to_fail, fail_allowed, "Fail allowed check");

                allowed_to_fail
            }
        }
    }

    fn to_undo(&self) -> Self {
        match self.clone() {
            Self::Enable {
                unit,
                scope,
                now,
                fail_allowed,
            } => Self::Disable {
                unit,
                scope,
                now,
                fail_allowed,
            },

            Self::Disable {
                unit,
                scope,
                now,
                fail_allowed,
            } => Self::Enable {
                unit,
                scope,
                now,
                fail_allowed,
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::assert_matches;

    use super::*;
    use tracing_test::traced_test;

    fn enable_action() -> SystemdAction {
        SystemdAction::Enable {
            unit: "some-unit".to_string(),
            scope: Scope::User,
            now: Some(true),
            fail_allowed: Some(false),
        }
    }

    fn disable_action() -> SystemdAction {
        SystemdAction::Disable {
            unit: "some-unit".to_string(),
            scope: Scope::User,
            now: Some(true),
            fail_allowed: Some(false),
        }
    }

    fn command_args(action: &SystemdAction) -> Vec<String> {
        action
            .get_command()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn serde_yaml_parses_enable_action() {
        let yaml = r"
        type: enable
        unit: some-unit
        scope: user
        now: true
        fail_allowed: false
        ";

        let action: SystemdAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            SystemdAction::Enable {
                unit: "some-unit".to_string(),
                scope: Scope::User,
                now: Some(true),
                fail_allowed: Some(false),
            }
        );
        assert_eq!(
            command_args(&action),
            vec!["--user", "enable", "--now", "some-unit"]
        );
        assert!(!action.fail_allowed());
        assert!(!action.needs_elevation());
    }

    #[test]
    fn serde_yaml_parses_disable_action() {
        let yaml = r"
        type: disable
        unit: some-unit
        scope: system
        now: false
        fail_allowed: true
        ";

        let action: SystemdAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            SystemdAction::Disable {
                unit: "some-unit".to_string(),
                scope: Scope::System,
                now: Some(false),
                fail_allowed: Some(true),
            }
        );
        assert_eq!(
            command_args(&action),
            vec!["--system", "disable", "some-unit"]
        );
        assert!(action.fail_allowed());
        assert!(action.needs_elevation());
    }

    #[test]
    fn serde_yaml_parses_optional_values() {
        let yaml = r"
        type: enable
        unit: some-unit
        scope: user
        ";

        let action: SystemdAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            SystemdAction::Enable {
                unit: "some-unit".to_string(),
                scope: Scope::User,
                now: None,
                fail_allowed: None,
            }
        );
        assert_eq!(
            command_args(&action),
            vec!["--user", "enable", "--now", "some-unit"]
        );
        assert!(!action.fail_allowed());
        assert!(!action.needs_elevation());
    }

    #[test]
    fn serde_yaml_round_trips_enable_action() {
        let action = enable_action();

        let yaml = serde_yaml::to_string(&action).unwrap();
        let parsed: SystemdAction = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed, action);
    }

    #[test]
    fn serde_yaml_round_trips_disable_action() {
        let action = disable_action();

        let yaml = serde_yaml::to_string(&action).unwrap();
        let parsed: SystemdAction = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed, action);
    }

    #[test]
    fn enable_status_maps_correctly() {
        let action = enable_action();

        assert_eq!(
            IsEnabledOutput::Enabled.to_action_state(&action, None, None),
            ActionState::Done
        );

        assert_eq!(
            IsEnabledOutput::Disabled.to_action_state(&action, None, None),
            ActionState::Available
        );

        assert_matches!(
            IsEnabledOutput::NotFound.to_action_state(&action, None, None),
            ActionState::UnAvailable(_)
        );
    }

    #[test]
    fn disable_status_maps_correctly() {
        let action = disable_action();

        assert_eq!(
            IsEnabledOutput::Disabled.to_action_state(&action, None, None),
            ActionState::Done
        );

        assert_eq!(
            IsEnabledOutput::Enabled.to_action_state(&action, None, None),
            ActionState::Available
        );

        assert_matches!(
            IsEnabledOutput::NotFound.to_action_state(&action, None, None),
            ActionState::UnAvailable(_)
        );
    }

    #[test]
    #[traced_test]
    fn enable_alias_reaches_max_recursion_depth() {
        let action = enable_action();

        assert_matches!(
            IsEnabledOutput::Alias.to_action_state(&action, None, Some(11)),
            ActionState::UnAvailable(_)
        );

        assert!(logs_contain("Reached max recursive depth"));
    }

    #[test]
    #[traced_test]
    fn disable_alias_reaches_max_recursion_depth() {
        let action = disable_action();

        assert_matches!(
            IsEnabledOutput::Alias.to_action_state(&action, None, Some(11)),
            ActionState::UnAvailable(_)
        );

        assert!(logs_contain("Reached max recursive depth"));
    }
}
