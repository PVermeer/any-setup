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

#[derive(Serialize, Deserialize, PartialEq, Hash, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SystemdAction {
    Enable { unit: String, scope: Scope },
    Disable { unit: String, scope: Scope },
}
impl Display for SystemdAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Enable { unit: name, scope } => {
                write!(f, "Systemd enable {scope} unit: {name}")
            }
            Self::Disable { unit: name, scope } => {
                write!(f, "Systemd disable {scope} unit: {name}")
            }
        }
    }
}
impl IsAction for SystemdAction {
    fn get_command(&self) -> Command {
        match self {
            Self::Enable { unit, scope } => {
                let mut command = Command::new("systemctl");
                command
                    .arg(scope.to_arg())
                    .arg("enable")
                    .arg("--now")
                    .arg(unit);

                command
            }

            Self::Disable { unit, scope } => {
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

    fn get_check_command(&self) -> Command {
        match self {
            Self::Enable { unit, scope } | Self::Disable { unit, scope } => {
                let mut command = Command::new("systemctl");
                command.arg(scope.to_arg()).arg("is-enabled").arg(unit);

                command
            }
        }
    }

    fn needs_elevation(&self) -> bool {
        match self {
            Self::Enable { unit: _, scope } | Self::Disable { unit: _, scope } => {
                *scope == Scope::System
            }
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

        debug!(action = %self, state = %stdout, "Action state");

        match is_enabled_output {
            IsEnabledOutput::Enabled => Ok(ActionState::Done),
            IsEnabledOutput::Disabled => Ok(ActionState::Available),
            _ => Ok(ActionState::UnAvailable),
        }
    }
}
