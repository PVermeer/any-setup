use super::{ActionState, OutputExt};
use crate::application::task_manager::{
    actions::IsAction, user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result, anyhow};
use common::{
    dbus_query::{self, DbusConnectionType, DbusPropertyQuery},
    utils,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Write},
    process::{Command, Output},
    sync::Arc,
};
use tracing::{debug, error};

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
            Self::Kargs { add, remove } => {
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

impl RpmOstreeAction {
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

            Self::Kargs { add, remove } => {
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

    #[allow(clippy::ref_option)]
    fn get_action_state_from_kargs(
        add: &Option<Vec<String>>,
        remove: &Option<Vec<String>>,
        stdout: &str,
    ) -> ActionState {
        let add_needed = add
            .as_ref()
            .is_some_and(|args| args.iter().any(|arg| !stdout.contains(arg)));

        let remove_needed = remove
            .as_ref()
            .is_some_and(|args| args.iter().any(|arg| stdout.contains(arg)));

        if add_needed || remove_needed {
            ActionState::Available
        } else {
            ActionState::Done
        }
    }
}

impl IsAction for RpmOstreeAction {
    fn run_action(
        &self,
        output: &mut Output,
        _user_context: &Arc<UserExecutionContext>,
        _needs_user_context: bool,
    ) -> Result<()> {
        let mut command = self.get_command();

        let command_output = command.output().context(match self {
            Self::Install { .. } => "Failed to run rpm-ostree install action",
            Self::Remove { .. } => "Failed to run rpm-ostree remove action",
            Self::Kargs { .. } => "Failed to run rpm-ostree kargs action",
        })?;

        output.append_output(command_output);

        Ok(())
    }

    fn needs_elevation(&self) -> bool {
        match self {
            Self::Install { .. } | Self::Remove { .. } => false,
            Self::Kargs { .. } => true,
        }
    }

    fn get_status(&self, _user_context: &Arc<UserExecutionContext>) -> ActionState {
        debug!(action = %self, "Running check command");

        if !utils::command::test_command_available_sync("rpm-ostree") {
            let message = "'rpm-ostree' Command not available";
            error!(message);
            return ActionState::UnAvailable(message.to_string());
        }
        if !utils::command::test_command_available_sync("rpm") {
            let message = "'rpm' Command not available";
            error!(message);
            return ActionState::UnAvailable(message.to_string());
        }

        let action_state = match self {
            Self::Install { packages, .. } => {
                let Ok(output) = Command::new("rpm").arg("-q").args(packages).output() else {
                    let message = "Failed to run 'rpm -q'";
                    error!(message);
                    return ActionState::UnAvailable(message.to_string());
                };

                if output.status.success() {
                    ActionState::Done
                } else {
                    ActionState::Available
                }
            }

            Self::Remove { packages, .. } => {
                let Ok(output) = Command::new("rpm").arg("-q").args(packages).output() else {
                    let message = "Failed to run 'rpm -q'";
                    error!(message);
                    return ActionState::UnAvailable(message.to_string());
                };

                if output.status.success() {
                    ActionState::Available
                } else {
                    ActionState::Done
                }
            }

            Self::Kargs { add, remove } => {
                let Ok(output) = Command::new("rpm-ostree").arg("kargs").output() else {
                    let message = "Failed to run 'rpm-ostree kargs'";
                    error!(message);
                    return ActionState::UnAvailable(message.to_string());
                };

                if !output.status.success() {
                    let message = "'rpm-ostree kargs' failed";
                    error!(
                        stderr = utils::command::parse_output(&output.stderr),
                        message
                    );
                    return ActionState::UnAvailable(message.to_string());
                }

                let stdout = utils::command::parse_output(&output.stdout);

                Self::get_action_state_from_kargs(add, remove, &stdout)
            }
        };

        debug!(action = %self, %action_state, "Current action state");

        action_state
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

    fn before_retry(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
        needs_user_context: bool,
    ) -> Option<Result<()>> {
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

        output.append_stdout("== Transaction in progress, canceling");

        let mut command = Command::new("rpm-ostree");
        command.arg("cancel");

        if needs_user_context {
            user_context.apply_to(&mut command);
        }

        match command.output() {
            Err(error) => {
                output.append_stderr(&error.to_string());
                Some(Err(anyhow!(error)))
            }

            Ok(cancel_output) => {
                output.append_output(cancel_output);
                Some(Ok(()))
            }
        }
    }

    fn needs_reboot(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn install_action_creates_correct_command() {
        let action = RpmOstreeAction::Install {
            packages: strings(&["foo", "bar"]),
            fail_allowed: Some(false),
        };

        let command = action.get_command();

        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["install", "foo bar"]
        );
    }

    #[test]
    fn remove_action_creates_correct_command() {
        let action = RpmOstreeAction::Remove {
            packages: strings(&["foo", "bar"]),
            fail_allowed: Some(false),
        };

        let command = action.get_command();

        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["remove", "foo bar"]
        );
    }

    #[test]
    fn kargs_action_creates_correct_command() {
        let action = RpmOstreeAction::Kargs {
            add: Some(strings(&["foo=bar", "baz"])),
            remove: Some(strings(&["quiet"])),
        };

        let command = action.get_command();

        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec![
                "kargs",
                "--append-if-missing=foo=bar",
                "--append-if-missing=baz",
                "--delete-if-present=quiet",
            ]
        );
    }

    #[test]
    fn kargs_action_supports_only_add() {
        let action = RpmOstreeAction::Kargs {
            add: Some(strings(&["foo=bar"])),
            remove: None,
        };

        assert_eq!(
            action
                .get_command()
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["kargs", "--append-if-missing=foo=bar"]
        );
    }

    #[test]
    fn kargs_action_supports_only_remove() {
        let action = RpmOstreeAction::Kargs {
            add: None,
            remove: Some(strings(&["quiet"])),
        };

        assert_eq!(
            action
                .get_command()
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["kargs", "--delete-if-present=quiet"]
        );
    }

    #[test]
    fn kargs_action_supports_neither_add_nor_remove() {
        let action = RpmOstreeAction::Kargs {
            add: None,
            remove: None,
        };

        assert_eq!(
            action
                .get_command()
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["kargs"]
        );
    }

    #[test]
    fn kargs_status_is_available_when_argument_is_missing() {
        assert_eq!(
            RpmOstreeAction::get_action_state_from_kargs(
                &Some(strings(&["amdgpu.ppfeaturemask=0xFFF7FFFF"])),
                &None,
                "quiet splash",
            ),
            ActionState::Available
        );
    }

    #[test]
    fn kargs_status_is_done_when_added_argument_exists() {
        assert_eq!(
            RpmOstreeAction::get_action_state_from_kargs(
                &Some(strings(&["amdgpu.ppfeaturemask=0xFFF7FFFF"])),
                &None,
                "quiet amdgpu.ppfeaturemask=0xFFF7FFFF splash",
            ),
            ActionState::Done
        );
    }

    #[test]
    fn kargs_status_is_available_when_removal_argument_exists() {
        assert_eq!(
            RpmOstreeAction::get_action_state_from_kargs(
                &None,
                &Some(strings(&["quiet"])),
                "quiet splash",
            ),
            ActionState::Available
        );
    }

    #[test]
    fn kargs_status_is_done_when_removal_argument_is_absent() {
        assert_eq!(
            RpmOstreeAction::get_action_state_from_kargs(
                &None,
                &Some(strings(&["quiet"])),
                "splash",
            ),
            ActionState::Done
        );
    }

    #[test]
    fn kargs_status_checks_all_requested_arguments() {
        assert_eq!(
            RpmOstreeAction::get_action_state_from_kargs(
                &Some(strings(&["foo=1", "bar=2"])),
                &None,
                "foo=1",
            ),
            ActionState::Available
        );
    }

    #[test]
    fn kargs_status_handles_add_and_remove_together() {
        assert_eq!(
            RpmOstreeAction::get_action_state_from_kargs(
                &Some(strings(&["foo=1"])),
                &Some(strings(&["quiet"])),
                "quiet splash",
            ),
            ActionState::Available
        );

        assert_eq!(
            RpmOstreeAction::get_action_state_from_kargs(
                &Some(strings(&["foo=1"])),
                &Some(strings(&["quiet"])),
                "foo=1 splash",
            ),
            ActionState::Done
        );
    }

    #[test]
    fn kargs_undo_swaps_add_and_remove() {
        let action = RpmOstreeAction::Kargs {
            add: Some(strings(&["foo=bar"])),
            remove: Some(strings(&["quiet"])),
        };

        assert!(matches!(
            action.to_undo(),
            RpmOstreeAction::Kargs {
                add: Some(add),
                remove: Some(remove),
            } if add == strings(&["quiet"]) && remove == strings(&["foo=bar"])
        ));
    }

    #[test]
    fn kargs_undo_of_add_becomes_remove() {
        let action = RpmOstreeAction::Kargs {
            add: Some(strings(&["amdgpu.ppfeaturemask=0xFFF7FFFF"])),
            remove: None,
        };

        let undo = action.to_undo();

        assert!(matches!(
            &undo,
            RpmOstreeAction::Kargs {
                add: None,
                remove: Some(remove),
            } if remove == &strings(&["amdgpu.ppfeaturemask=0xFFF7FFFF"])
        ));

        if let RpmOstreeAction::Kargs { add, remove } = undo {
            assert_eq!(
                RpmOstreeAction::get_action_state_from_kargs(
                    &add,
                    &remove,
                    "quiet amdgpu.ppfeaturemask=0xFFF7FFFF",
                ),
                ActionState::Available
            );

            assert_eq!(
                RpmOstreeAction::get_action_state_from_kargs(&add, &remove, "quiet"),
                ActionState::Done
            );
        }
    }

    #[test]
    fn install_and_remove_are_undo_operations_of_each_other() {
        let install = RpmOstreeAction::Install {
            packages: strings(&["foo"]),
            fail_allowed: Some(true),
        };

        assert!(matches!(
            install.to_undo(),
            RpmOstreeAction::Remove {
                packages,
                fail_allowed: Some(true),
            } if packages == strings(&["foo"])
        ));

        let remove = RpmOstreeAction::Remove {
            packages: strings(&["bar"]),
            fail_allowed: Some(false),
        };

        assert!(matches!(
            remove.to_undo(),
            RpmOstreeAction::Install {
                packages,
                fail_allowed: Some(false),
            } if packages == strings(&["bar"])
        ));
    }

    #[test]
    fn action_metadata_is_correct() {
        let install = RpmOstreeAction::Install {
            packages: strings(&["foo"]),
            fail_allowed: Some(true),
        };

        let remove = RpmOstreeAction::Remove {
            packages: strings(&["foo"]),
            fail_allowed: Some(false),
        };

        let kargs = RpmOstreeAction::Kargs {
            add: Some(strings(&["foo=bar"])),
            remove: None,
        };

        assert!(!install.needs_elevation());
        assert!(!remove.needs_elevation());
        assert!(kargs.needs_elevation());

        assert!(install.fail_allowed());
        assert!(!remove.fail_allowed());
        assert!(!kargs.fail_allowed());

        assert!(install.needs_reboot());
        assert!(remove.needs_reboot());
        assert!(kargs.needs_reboot());
    }
}
