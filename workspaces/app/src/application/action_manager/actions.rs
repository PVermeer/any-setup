pub mod systemd;

use serde::{Deserialize, Serialize};
use std::{fmt::Display, process::Command};
use systemd::SystemdAction;

pub trait IsAction: Display {
    fn get_command(&self) -> Command;
    fn needs_elevation(&self) -> bool;
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum Action {
    SystemD(SystemdAction),
}
impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SystemD(action) => action.fmt(f),
        }
    }
}
impl IsAction for Action {
    fn get_command(&self) -> Command {
        match self {
            Self::SystemD(action) => action.get_command(),
        }
    }

    fn needs_elevation(&self) -> bool {
        match self {
            Self::SystemD(action) => action.needs_elevation(),
        }
    }
}
