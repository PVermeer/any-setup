use crate::application::action_manager::actions::IsAction;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, process::Command};

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
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

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
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
            Self::Enable { unit: name, scope } => {
                let mut command = Command::new("systemctl");
                command
                    .arg(scope.to_arg())
                    .arg("enable")
                    .arg("--now")
                    .arg(name);

                command
            }

            Self::Disable { unit: name, scope } => {
                let mut command = Command::new("systemctl");
                command
                    .arg(scope.to_arg())
                    .arg("disable")
                    .arg("--now")
                    .arg(name);

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
}
