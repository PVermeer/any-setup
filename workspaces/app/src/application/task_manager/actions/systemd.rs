use super::ActionState;
use crate::application::task_manager::actions::IsAction;
use anyhow::{Context, Result, anyhow};
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
    fn to_action_state(&self, action: &SystemdAction) -> ActionState {
        match action {
            SystemdAction::Enable { .. } => match self {
                Self::Enabled => ActionState::Done,
                Self::Disabled => ActionState::Available,
                _ => ActionState::UnAvailable,
            },

            SystemdAction::Disable { .. } => match self {
                Self::Disabled => ActionState::Done,
                Self::Enabled => ActionState::Available,
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
        fail_allowed: Option<bool>,
    },
    Disable {
        unit: String,
        scope: Scope,
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
            Self::Enable { unit, scope, .. } => {
                let mut command = Command::new("systemctl");
                command
                    .arg(scope.to_arg())
                    .arg("enable")
                    .arg("--now")
                    .arg(unit);

                command
            }

            Self::Disable { unit, scope, .. } => {
                let mut command = Command::new("systemctl");
                command
                    .arg(scope.to_arg())
                    .arg("disable")
                    .arg("--now")
                    .arg(unit);

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
        debug!(action = %self, "Running check command");

        let output = self
            .get_check_command()
            .output()
            .context("Failed to run command")?;

        // Cannot test for success, it will be non-zero for disabled
        let stdout = utils::command::parse_output(&output.stdout);
        let is_enabled_output = IsEnabledOutput::from_str(&stdout)?;
        let action_state = is_enabled_output.to_action_state(self);

        debug!(action = %self, state = %stdout, action_state = %action_state, "Action state");

        Ok(action_state)
    }

    fn fail_allowed(&self) -> bool {
        match self {
            Self::Enable { fail_allowed, .. } | Self::Disable { fail_allowed, .. } => {
                fail_allowed.is_some_and(|fail_allowed| fail_allowed)
            }
        }
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn enable_action() -> SystemdAction {
        SystemdAction::Enable {
            unit: "some-unit".to_string(),
            scope: Scope::User,
            fail_allowed: Some(false),
        }
    }

    fn disable_action() -> SystemdAction {
        SystemdAction::Disable {
            unit: "some-unit".to_string(),
            scope: Scope::User,
            fail_allowed: Some(false),
        }
    }

    #[test]
    fn serde_yaml_parses_enable_action() {
        let yaml = r"
            type: enable
            unit: some-unit
            scope: user
            fail_allowed: false
            ";

        let action: SystemdAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            SystemdAction::Enable {
                unit: "some-unit".to_string(),
                scope: Scope::User,
                fail_allowed: Some(false),
            }
        );
    }

    #[test]
    fn serde_yaml_parses_disable_action() {
        let yaml = r"
            type: disable
            unit: some-unit
            scope: system
            fail_allowed: true
            ";

        let action: SystemdAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            SystemdAction::Disable {
                unit: "some-unit".to_string(),
                scope: Scope::System,
                fail_allowed: Some(true),
            }
        );
    }

    #[test]
    fn serde_yaml_parses_missing_fail_allowed() {
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
                fail_allowed: None,
            }
        );
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
            IsEnabledOutput::Enabled.to_action_state(&action),
            ActionState::Done
        );

        assert_eq!(
            IsEnabledOutput::Disabled.to_action_state(&action),
            ActionState::Available
        );

        assert_eq!(
            IsEnabledOutput::NotFound.to_action_state(&action),
            ActionState::UnAvailable
        );
    }

    #[test]
    fn disable_status_maps_correctly() {
        let action = disable_action();

        assert_eq!(
            IsEnabledOutput::Disabled.to_action_state(&action),
            ActionState::Done
        );

        assert_eq!(
            IsEnabledOutput::Enabled.to_action_state(&action),
            ActionState::Available
        );

        assert_eq!(
            IsEnabledOutput::NotFound.to_action_state(&action),
            ActionState::UnAvailable
        );
    }
}
