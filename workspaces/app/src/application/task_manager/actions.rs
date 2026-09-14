pub mod rpm_ostree;
pub mod systemd;

use anyhow::Result;
use rpm_ostree::RpmOstreeAction;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    process::{Command, Output},
};
use systemd::SystemdAction;

#[derive(Default, PartialEq, Debug)]
pub enum ActionState {
    Done,
    Available,
    #[default]
    UnAvailable,
}
impl Display for ActionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Done => write!(f, "done"),
            Self::Available => write!(f, "available"),
            Self::UnAvailable => write!(f, "unavailable"),
        }
    }
}

pub trait IsAction: Display {
    fn get_command(&self) -> Command;
    fn needs_elevation(&self) -> bool;
    fn get_status(&self) -> Result<ActionState>;
    fn fail_allowed(&self) -> bool;
    fn to_undo(&self) -> Self;
    fn on_error(&self, output: &Output) -> Option<Command>;
}

#[derive(Serialize, Deserialize, PartialEq, Hash, Clone, Debug)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum Action {
    SystemD(SystemdAction),
    RpmOstree(RpmOstreeAction),
}
impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SystemD(action) => action.fmt(f),
            Self::RpmOstree(action) => action.fmt(f),
        }
    }
}
impl IsAction for Action {
    fn get_command(&self) -> Command {
        match self {
            Self::SystemD(action) => action.get_command(),
            Self::RpmOstree(action) => action.get_command(),
        }
    }

    fn needs_elevation(&self) -> bool {
        match self {
            Self::SystemD(action) => action.needs_elevation(),
            Self::RpmOstree(action) => action.needs_elevation(),
        }
    }

    fn get_status(&self) -> Result<ActionState> {
        match self {
            Self::SystemD(action) => action.get_status(),
            Self::RpmOstree(action) => action.get_status(),
        }
    }

    fn fail_allowed(&self) -> bool {
        match self {
            Self::SystemD(action) => action.fail_allowed(),
            Self::RpmOstree(action) => action.fail_allowed(),
        }
    }

    fn to_undo(&self) -> Self {
        match self {
            Self::SystemD(action) => Self::SystemD(action.to_undo()),
            Self::RpmOstree(action) => Self::RpmOstree(action.to_undo()),
        }
    }

    fn on_error(&self, output: &Output) -> Option<Command> {
        match self {
            Self::SystemD(action) => action.on_error(output),
            Self::RpmOstree(action) => action.on_error(output),
        }
    }
}
