use super::ActionState;
use crate::application::task_manager::actions::IsAction;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, process::Command};
use tracing::debug;

#[derive(Debug)]
struct RpmStatus {
    installed: bool,
    removed: bool,
}
impl RpmStatus {
    fn to_action_state(&self) -> ActionState {
        match (self.installed, self.removed) {
            (true, true) => ActionState::Done,
            (false, false) => ActionState::Available,
            _ => ActionState::UnAvailable,
        }
    }
}

#[derive(Debug)]
struct RpmCommands {
    install: Option<Command>,
    remove: Option<Command>,
}
impl RpmCommands {
    fn get_status(self) -> Result<RpmStatus> {
        let is_installed = self
            .install
            .map(|mut command| {
                command
                    .status()
                    .context("Failed to run command")
                    .map(|status| status.success())
            })
            .transpose()?
            .unwrap_or(true);

        let is_removed = self
            .remove
            .map(|mut command| {
                command
                    .status()
                    .context("Failed to run command")
                    .map(|status| !status.success())
            })
            .transpose()?
            .unwrap_or(true);

        Ok(RpmStatus {
            installed: is_installed,
            removed: is_removed,
        })
    }
}

#[derive(Serialize, Deserialize, PartialEq, Hash, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RpmOstreeAction {
    Install {
        packages: Vec<String>,
        fail_allowed: Option<bool>,
    },
    Remove {
        packages: Vec<String>,
        fail_allowed: Option<bool>,
    },
    Compound {
        install: Vec<String>,
        remove: Vec<String>,
        fail_allowed: Option<bool>,
    },
}
impl Display for RpmOstreeAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Install { packages, .. } => {
                write!(f, "Rpm Ostree installing: {}.", packages.join(","))
            }
            Self::Remove { packages, .. } => {
                write!(f, "Rpm Ostree removing: {}.", packages.join(","))
            }
            Self::Compound {
                install, remove, ..
            } => {
                write!(
                    f,
                    "Rpm Ostree removing: {}.\nRpm Ostree installing {}.",
                    install.join(","),
                    remove.join(",")
                )
            }
        }
    }
}
impl IsAction for RpmOstreeAction {
    fn get_command(&self) -> Command {
        match self {
            Self::Install { packages, .. } => {
                let mut command = Command::new("rpm-ostree");
                command.arg("install").arg(packages.join(" "));

                command
            }

            Self::Remove { packages, .. } => {
                let mut command = Command::new("rpm-ostree");
                command.arg("remove").arg(packages.join(" "));

                command
            }

            Self::Compound {
                install, remove, ..
            } => {
                let mut command = Command::new("rpm-ostree");
                command
                    .arg("remove")
                    .arg(remove.join(" "))
                    .arg("--install")
                    .arg(install.join(" "));

                command
            }
        }
    }

    fn needs_elevation(&self) -> bool {
        false
    }

    fn get_status(&self) -> Result<ActionState> {
        debug!(action = %self, "Running check command");

        let status = self.get_check_commands().get_status()?;
        let action_state = status.to_action_state();

        debug!(
            action = %self,
            is_installed = status.installed,
            is_removed = status.removed,
            action_state = %action_state,
            "Action state"
        );

        Ok(action_state)
    }

    fn fail_allowed(&self) -> bool {
        match self {
            Self::Install { fail_allowed, .. }
            | Self::Remove { fail_allowed, .. }
            | Self::Compound { fail_allowed, .. } => {
                fail_allowed.is_some_and(|fail_allowed| fail_allowed)
            }
        }
    }
}
impl RpmOstreeAction {
    fn get_check_commands(&self) -> RpmCommands {
        match self {
            Self::Install { packages, .. } => {
                let mut command = Command::new("rpm");
                command.arg("-q").arg(packages.join(" "));

                RpmCommands {
                    install: Some(command),
                    remove: None,
                }
            }

            Self::Remove { packages, .. } => {
                let mut command = Command::new("rpm");
                command.arg("-q").arg(packages.join(" "));

                RpmCommands {
                    install: None,
                    remove: Some(command),
                }
            }

            Self::Compound {
                install,
                remove,
                fail_allowed,
            } => {
                let install_command = Self::Install {
                    packages: install.to_owned(),
                    fail_allowed: *fail_allowed,
                }
                .get_check_commands();

                let remove_command = Self::Remove {
                    packages: remove.to_owned(),
                    fail_allowed: *fail_allowed,
                }
                .get_check_commands();

                RpmCommands {
                    install: install_command.install,
                    remove: remove_command.remove,
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn rpm_status_maps_to_action_state_correctly() {
        assert_eq!(
            RpmStatus {
                installed: true,
                removed: true,
            }
            .to_action_state(),
            ActionState::Done
        );

        assert_eq!(
            RpmStatus {
                installed: false,
                removed: false,
            }
            .to_action_state(),
            ActionState::Available
        );

        assert_eq!(
            RpmStatus {
                installed: true,
                removed: false,
            }
            .to_action_state(),
            ActionState::UnAvailable
        );

        assert_eq!(
            RpmStatus {
                installed: false,
                removed: true,
            }
            .to_action_state(),
            ActionState::UnAvailable
        );
    }

    #[test]
    fn install_action_creates_installed_check() {
        let action = RpmOstreeAction::Install {
            packages: Vec::from(["foo".to_string()]),
            fail_allowed: Some(false),
        };

        let commands = action.get_check_commands();

        assert!(commands.install.is_some());
        assert!(commands.remove.is_none());
    }

    #[test]
    fn remove_action_creates_removed_check() {
        let action = RpmOstreeAction::Remove {
            packages: Vec::from(["foo".to_string()]),
            fail_allowed: Some(false),
        };

        let commands = action.get_check_commands();

        assert!(commands.install.is_none());
        assert!(commands.remove.is_some());
    }

    #[test]
    fn compound_action_creates_both_checks() {
        let action = RpmOstreeAction::Compound {
            install: Vec::from(["foo".to_string()]),
            remove: Vec::from(["bar".to_string()]),
            fail_allowed: Some(false),
        };

        let commands = action.get_check_commands();

        assert!(commands.install.is_some());
        assert!(commands.remove.is_some());
    }

    #[test]
    fn rpm_commands_with_no_checks_are_treated_as_true() {
        let status = RpmCommands {
            install: None,
            remove: None,
        }
        .get_status()
        .unwrap();

        assert!(status.installed);
        assert!(status.removed);
    }
}
