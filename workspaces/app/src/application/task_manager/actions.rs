#[macro_use]
mod action_macro;
pub mod rpm_ostree;
pub mod systemd;
pub mod user_group;

use super::user_execution_context::UserExecutionContext;
use anyhow::Result;
use rpm_ostree::RpmOstreeAction;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    process::{Command, Output},
};
use systemd::SystemdAction;
use user_group::UserGroupAction;

#[derive(Default, Debug)]
#[cfg_attr(test, derive(PartialEq))]
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
    fn get_command(&self, user_context: &UserExecutionContext) -> Command;
    fn needs_elevation(&self) -> bool;
    fn get_status(&self, user_context: &UserExecutionContext) -> Result<ActionState>;
    fn fail_allowed(&self) -> bool;
    fn to_undo(&self) -> Self;
    fn on_error(&self, output: &Output) -> Option<Command>;
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum Action {
    SystemD(SystemdAction),
    RpmOstree(RpmOstreeAction),
    UserGroup(UserGroupAction),
}
// Using macro to impl because it's just a function map of IsAction to all the Actions
impl_action! {
    Action {
        SystemD,
        RpmOstree,
        UserGroup,
    }
}
