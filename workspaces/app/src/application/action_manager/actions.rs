pub mod systemd;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, process::Command};
use systemd::SystemdAction;

#[derive(PartialEq)]
pub enum ActionState {
    Done,
    Available,
    UnAvailable,
}
impl Default for ActionState {
    fn default() -> Self {
        ActionState::UnAvailable
    }
}

pub trait IsAction: Display {
    fn get_command(&self) -> Command;
    fn get_check_command(&self) -> Command;
    fn needs_elevation(&self) -> bool;
    fn get_status(&self) -> Result<ActionState>;
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

    fn get_check_command(&self) -> Command {
        match self {
            Self::SystemD(action) => action.get_check_command(),
        }
    }

    fn needs_elevation(&self) -> bool {
        match self {
            Self::SystemD(action) => action.needs_elevation(),
        }
    }

    fn get_status(&self) -> Result<ActionState> {
        match self {
            Self::SystemD(action) => action.get_status(),
        }
    }
}
