#[macro_use]
mod action_macro;
pub mod flatpak;
pub mod rpm_ostree;
pub mod systemd;
pub mod udev;
pub mod user_group;

use super::user_execution_context::UserExecutionContext;
use anyhow::Result;
use flatpak::FlatpakAction;
use rpm_ostree::RpmOstreeAction;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, process::Output, sync::Arc};
use systemd::SystemdAction;
use udev::UdevAction;
use user_group::UserGroupAction;

/// Extend Output with some convenience methods
pub trait OutputExt {
    fn append_stdout(&mut self, message: &str);
    fn append_stderr(&mut self, message: &str);
    fn append_output(&mut self, output: Output);
}
impl OutputExt for Output {
    fn append_stdout(&mut self, message: &str) {
        self.stdout
            .extend_from_slice(format!("{message}\n").as_bytes());
    }

    fn append_stderr(&mut self, message: &str) {
        self.stderr
            .extend_from_slice(format!("{message}\n").as_bytes());
    }

    fn append_output(&mut self, output: Output) {
        self.stdout.extend(output.stdout);
        self.stderr.extend(output.stderr);
        self.status = output.status;
    }
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
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
impl ActionState {
    pub fn to_log_message(&self) -> String {
        format!("Action is {self}.")
    }
}

pub trait IsAction: Display {
    fn run_action(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
        needs_user_contex: bool,
    ) -> Result<()>;
    fn needs_elevation(&self) -> bool;
    fn get_status(&self, user_context: &Arc<UserExecutionContext>) -> Result<ActionState>;
    fn fail_allowed(&self) -> bool;
    fn to_undo(&self) -> Self;
    /// This function runs before a retry is attempted
    fn before_retry(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
        needs_user_contex: bool,
    ) -> Option<Result<()>>;
    fn needs_reboot(&self) -> bool;
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum Action {
    SystemD(SystemdAction),
    RpmOstree(RpmOstreeAction),
    UserGroup(UserGroupAction),
    Flatpak(FlatpakAction),
    Udev(UdevAction),
}
// Using macro to impl because it's just a function map of IsAction to all the Actions
impl_action! {
    Action {
        SystemD,
        RpmOstree,
        UserGroup,
        Flatpak,
        Udev
    }
}
