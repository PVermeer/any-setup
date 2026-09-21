use super::ActionState;
use crate::application::task_manager::{
    actions::IsAction, user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result};
use common::{
    dbus_query::{self, DbusConnectionType, DbusPropertyQuery},
    utils,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::{fmt::Display, process::Command};
use tracing::debug;

#[derive(Debug)]
struct KargsCommand {
    command: Command,
    add: Option<Vec<String>>,
    remove: Option<Vec<String>>,
}

#[derive(Debug)]
struct RpmStatus {
    installed: bool,
    removed: bool,
    kargs: bool,
}
impl RpmStatus {
    fn to_action_state(&self) -> ActionState {
        match (self.installed, self.removed, self.kargs) {
            (true, true, true) => ActionState::Done,
            (false, false, false) => ActionState::Available,
            _ => ActionState::UnAvailable,
        }
    }
}

#[derive(Debug)]
struct RpmCommands {
    install: Option<Command>,
    remove: Option<Command>,
    kargs: Option<KargsCommand>,
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

        let kargs = self
            .kargs
            .map(|mut kargs_command| {
                kargs_command
                    .command
                    .output()
                    .context("Failed to run command")
                    .map(|output| {
                        let stdout = utils::command::parse_output(&output.stdout);

                        let add_needed = kargs_command
                            .add
                            .is_some_and(|add| add.iter().any(|karg| !stdout.contains(karg)));

                        let remove_needed = kargs_command
                            .remove
                            .is_some_and(|remove| remove.iter().any(|karg| stdout.contains(karg)));

                        !(add_needed || remove_needed)
                    })
            })
            .transpose()?
            .unwrap_or(true);

        Ok(RpmStatus {
            installed: is_installed,
            removed: is_removed,
            kargs,
        })
    }
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
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
    Kargs {
        add: Option<Vec<String>>,
        remove: Option<Vec<String>>,
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
            Self::Kargs { add, remove, .. } => {
                let mut fmt_string = String::new();

                if let Some(remove) = remove {
                    let _ = writeln!(
                        fmt_string,
                        "Rpm Ostree removing kargs: {}",
                        remove.join(" ")
                    );
                }
                if let Some(add) = add {
                    let _ = write!(fmt_string, "Rpm Ostree adding kargs: {}", add.join(" "));
                }

                write!(f, "{fmt_string}")
            }
        }
    }
}
impl IsAction for RpmOstreeAction {
    fn get_command(&self, _user_context: &UserExecutionContext) -> Command {
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

            Self::Kargs { add, remove, .. } => {
                let mut command = Command::new("rpm-ostree");
                command.arg("kargs");

                if let Some(add) = add {
                    for karg in add {
                        command.arg(format!("--append-if-missing={karg}"));
                    }
                }
                if let Some(remove) = remove {
                    for karg in remove {
                        command.arg(format!("--delete-if-present={karg}"));
                    }
                }

                command
            }
        }
    }

    fn needs_elevation(&self) -> bool {
        match self {
            Self::Install { .. } | Self::Remove { .. } => false,
            Self::Kargs { .. } => true,
        }
    }

    fn get_status(&self, _user_context: &UserExecutionContext) -> Result<ActionState> {
        debug!(action = %self, "Running check command");

        let status = self.get_check_commands().get_status()?;
        let action_state = status.to_action_state();

        debug!(
            action = %self,
            is_installed = status.installed,
            is_removed = status.removed,
            kargs = status.kargs,
            action_state = %action_state,
            "Action state"
        );

        Ok(action_state)
    }

    fn fail_allowed(&self) -> bool {
        match self {
            Self::Install { fail_allowed, .. } | Self::Remove { fail_allowed, .. } => {
                fail_allowed.is_some_and(|fail_allowed| fail_allowed)
            }
            Self::Kargs { .. } => false,
        }
    }

    fn to_undo(&self) -> Self {
        match self.clone() {
            Self::Install {
                packages,
                fail_allowed,
            } => Self::Remove {
                packages,
                fail_allowed,
            },

            Self::Remove {
                packages,
                fail_allowed,
            } => Self::Install {
                packages,
                fail_allowed,
            },

            Self::Kargs { add, remove } => Self::Kargs {
                add: remove,
                remove: add,
            },
        }
    }

    fn on_error(&self, _output: &std::process::Output) -> Option<Command> {
        let rpm_ostree_is_idle = dbus_query::get_property::<String>(DbusPropertyQuery {
            connection_type: DbusConnectionType::System,
            destination: "org.projectatomic.rpmostree1",
            path: "/org/projectatomic/rpmostree1/Sysroot",
            interface: "org.projectatomic.rpmostree1.Sysroot",
            property: "ActiveTransactionPath",
        })
        .map(|property| property.is_empty())
        .ok()?;

        if rpm_ostree_is_idle {
            return None;
        }

        match self {
            Self::Install { .. } | Self::Remove { .. } | Self::Kargs { .. } => {
                let mut command = Command::new("rpm-ostree");
                command.arg("cancel");

                Some(command)
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
                    kargs: None,
                }
            }

            Self::Remove { packages, .. } => {
                let mut command = Command::new("rpm");
                command.arg("-q").arg(packages.join(" "));

                RpmCommands {
                    install: None,
                    remove: Some(command),
                    kargs: None,
                }
            }

            Self::Kargs { add, remove, .. } => {
                let mut command = Command::new("rpm-ostree");
                command.arg("kargs");

                RpmCommands {
                    install: None,
                    remove: None,
                    kargs: Some(KargsCommand {
                        command,
                        add: add.clone(),
                        remove: remove.clone(),
                    }),
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
        for installed in [false, true] {
            for removed in [false, true] {
                for kargs in [false, true] {
                    let expected = match (installed, removed, kargs) {
                        // Only these matches should result in a state other than UnAvailable
                        (true, true, true) => ActionState::Done,
                        (false, false, false) => ActionState::Available,
                        _ => ActionState::UnAvailable,
                    };

                    assert_eq!(
                        RpmStatus {
                            installed,
                            removed,
                            kargs,
                        }
                        .to_action_state(),
                        expected,
                        "unexpected state for installed={installed}, removed={removed}, kargs={kargs}"
                    );
                }
            }
        }
    }

    #[test]
    fn install_action_creates_install_check() {
        let action = RpmOstreeAction::Install {
            packages: Vec::from(["foo".to_string()]),
            fail_allowed: Some(false),
        };

        let commands = action.get_check_commands();

        assert!(commands.install.is_some());
        assert!(commands.remove.is_none());
        assert!(commands.kargs.is_none());
    }

    #[test]
    fn remove_action_creates_remove_check() {
        let action = RpmOstreeAction::Remove {
            packages: Vec::from(["foo".to_string()]),
            fail_allowed: Some(false),
        };

        let commands = action.get_check_commands();

        assert!(commands.install.is_none());
        assert!(commands.remove.is_some());
        assert!(commands.kargs.is_none());
    }

    #[test]
    fn kargs_action_creates_kargs_check() {
        let action = RpmOstreeAction::Kargs {
            add: Some(Vec::from(["foo=bar".to_string()])),
            remove: Some(Vec::from(["quiet".to_string()])),
        };

        let commands = action.get_check_commands();

        assert!(commands.install.is_none());
        assert!(commands.remove.is_none());
        assert!(commands.kargs.is_some());

        let kargs = commands.kargs.unwrap();

        assert_eq!(kargs.add, Some(Vec::from(["foo=bar".to_string()])));
        assert_eq!(kargs.remove, Some(Vec::from(["quiet".to_string()])));
    }

    #[test]
    fn kargs_action_supports_only_add() {
        let action = RpmOstreeAction::Kargs {
            add: Some(Vec::from(["foo=bar".to_string()])),
            remove: None,
        };

        let commands = action.get_check_commands();
        let kargs = commands.kargs.unwrap();

        assert_eq!(kargs.add, Some(Vec::from(["foo=bar".to_string()])));
        assert!(kargs.remove.is_none());
    }

    #[test]
    fn kargs_action_supports_only_remove() {
        let action = RpmOstreeAction::Kargs {
            add: None,
            remove: Some(Vec::from(["quiet".to_string()])),
        };

        let commands = action.get_check_commands();
        let kargs = commands.kargs.unwrap();

        assert!(kargs.add.is_none());
        assert_eq!(kargs.remove, Some(Vec::from(["quiet".to_string()])));
    }

    #[test]
    fn kargs_action_supports_neither_add_nor_remove() {
        let action = RpmOstreeAction::Kargs {
            add: None,
            remove: None,
        };

        let commands = action.get_check_commands();
        let kargs = commands.kargs.unwrap();

        assert!(kargs.add.is_none());
        assert!(kargs.remove.is_none());
    }

    #[test]
    fn kargs_action_creates_correct_command() {
        let user_context = UserExecutionContext::new().unwrap();

        let action = RpmOstreeAction::Kargs {
            add: Some(Vec::from(["foo=bar".to_string(), "baz".to_string()])),
            remove: Some(Vec::from(["quiet".to_string()])),
        };

        let command = action.get_command(&user_context);

        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![
                "kargs",
                "--append-if-missing=foo=bar",
                "--append-if-missing=baz",
                "--delete-if-present=quiet",
            ]
        );
    }

    #[test]
    fn rpm_commands_with_no_checks_are_treated_as_true() {
        let status = RpmCommands {
            install: None,
            remove: None,
            kargs: None,
        }
        .get_status()
        .unwrap();

        assert!(status.installed);
        assert!(status.removed);
        assert!(status.kargs);
        assert_eq!(status.to_action_state(), ActionState::Done);
    }
}
