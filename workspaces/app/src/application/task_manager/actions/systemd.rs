use super::ActionState;
use crate::application::task_manager::actions::IsAction;
use anyhow::{Context, Result, anyhow, bail};
use common::utils;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, process::Command, str::FromStr};
use tracing::{debug, error};

#[derive(Serialize, Deserialize, PartialEq, Hash, Clone, Debug)]
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

/// Enum for mapping systemctl is-enabled output
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
    fn from_action(action: &SystemdAction) -> Result<Self> {
        let output = action
            .get_check_command()
            .output()
            .context("Failed to run command")?;
        let stdout = utils::command::parse_output(&output.stdout);

        Self::from_str(&stdout)
    }

    fn to_action_state(&self, action: &SystemdAction, recursive_depth: Option<u32>) -> ActionState {
        fn handle_alias(action: &SystemdAction, recursive_depth: Option<u32>) -> ActionState {
            let depth = recursive_depth.unwrap_or(0);
            if depth > 10 {
                error!(%action, depth, "Reached max recursive depth trying to resolve an alias");
                return ActionState::UnAvailable;
            }

            let Ok(resolved_unit_name) = action.resolve_alias() else {
                error!("Failed to resolve alias for");
                return ActionState::UnAvailable;
            };
            let mut action_clone = action.clone();
            match &mut action_clone {
                SystemdAction::Enable { unit, .. } | SystemdAction::Disable { unit, .. } => {
                    *unit = resolved_unit_name;
                }
            }
            let Ok(new_is_enabled_output) = IsEnabledOutput::from_action(&action_clone) else {
                return ActionState::UnAvailable;
            };

            new_is_enabled_output.to_action_state(&action_clone, Some(depth + 1))
        }

        match action {
            SystemdAction::Enable { .. } => match self {
                Self::Enabled => ActionState::Done,
                Self::Disabled => ActionState::Available,
                Self::Alias => handle_alias(action, recursive_depth),
                _ => ActionState::UnAvailable,
            },

            SystemdAction::Disable { .. } => match self {
                Self::Enabled => ActionState::Available,
                Self::Disabled => ActionState::Done,
                Self::Alias => handle_alias(action, recursive_depth),
                _ => ActionState::UnAvailable,
            },
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Hash, Clone, Debug)]
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
            } => {
                write!(f, "Systemd enable {scope} unit: {name}")
            }
            Self::Disable {
                unit: name, scope, ..
            } => {
                write!(f, "Systemd disable {scope} unit: {name}")
            }
        }
    }
}
impl IsAction for SystemdAction {
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

    fn needs_elevation(&self) -> bool {
        match self {
            Self::Enable { scope, .. } | Self::Disable { scope, .. } => *scope == Scope::System,
        }
    }

    fn get_status(&self) -> Result<ActionState> {
        debug!(action = %self, "Running status command");

        let is_enabled_output = IsEnabledOutput::from_action(self)?;
        let action_state = is_enabled_output.to_action_state(self, None);

        debug!(action = %self, is_enabled_output = %is_enabled_output, action_state = %action_state, "Action state");

        Ok(action_state)
    }

    fn fail_allowed(&self) -> bool {
        match self {
            Self::Enable { fail_allowed, .. } | Self::Disable { fail_allowed, .. } => {
                fail_allowed.is_some_and(|fail_allowed| fail_allowed)
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

    fn on_error(&self, _output: &std::process::Output) -> Option<Command> {
        None
    }
}
impl SystemdAction {
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
        match self {
            SystemdAction::Enable { unit, scope, .. }
            | SystemdAction::Disable { unit, scope, .. } => {
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

                Ok(utils::command::parse_output(&output.stdout))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
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
            IsEnabledOutput::Enabled.to_action_state(&action, None),
            ActionState::Done
        );

        assert_eq!(
            IsEnabledOutput::Disabled.to_action_state(&action, None),
            ActionState::Available
        );

        assert_eq!(
            IsEnabledOutput::NotFound.to_action_state(&action, None),
            ActionState::UnAvailable
        );
    }

    #[test]
    fn disable_status_maps_correctly() {
        let action = disable_action();

        assert_eq!(
            IsEnabledOutput::Disabled.to_action_state(&action, None),
            ActionState::Done
        );

        assert_eq!(
            IsEnabledOutput::Enabled.to_action_state(&action, None),
            ActionState::Available
        );

        assert_eq!(
            IsEnabledOutput::NotFound.to_action_state(&action, None),
            ActionState::UnAvailable
        );
    }

    #[test]
    #[traced_test]
    fn enable_alias_reaches_max_recursion_depth() {
        let action = enable_action();

        assert_eq!(
            IsEnabledOutput::Alias.to_action_state(&action, Some(11)),
            ActionState::UnAvailable
        );

        assert!(logs_contain("Reached max recursive depth"));
    }

    #[test]
    #[traced_test]
    fn disable_alias_reaches_max_recursion_depth() {
        let action = disable_action();

        assert_eq!(
            IsEnabledOutput::Alias.to_action_state(&action, Some(11)),
            ActionState::UnAvailable
        );

        assert!(logs_contain("Reached max recursive depth"));
    }
}
