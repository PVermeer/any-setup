use crate::application::actions::IsAction;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, process::Command};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
pub enum SystemdAction {
    EnableUnit { name: String, scope: Scope },
    DisableUnit { name: String, scope: Scope },
}
impl IsAction for SystemdAction {
    fn get_command(&self) -> Command {
        match self {
            Self::EnableUnit { name, scope } => {
                let mut command = Command::new("systemctl");
                command
                    .arg(scope.to_arg())
                    .arg("enable")
                    .arg("--now")
                    .arg(name);

                command
            }

            Self::DisableUnit { name, scope } => {
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
            Self::EnableUnit { name: _, scope } | Self::DisableUnit { name: _, scope } => {
                *scope == Scope::System
            }
        }
    }
}
