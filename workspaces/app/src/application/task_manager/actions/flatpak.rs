use super::ActionState;
use crate::application::task_manager::{
    actions::IsAction, user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, process::Command, sync::Arc};
use tracing::debug;

#[derive(Serialize, Deserialize, Hash, Clone, Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    System,
    User,
}
impl Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

#[derive(Serialize, Deserialize, Hash, Clone, Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "lowercase")]
pub enum Repo {
    #[default]
    Flathub,
    Fedora,
}
impl Display for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flathub => write!(f, "flathub"),
            Self::Fedora => write!(f, "fedora"),
        }
    }
}
impl Repo {
    fn to_arg(&self) -> String {
        self.to_string()
    }
}

enum InfoOutput {
    Found,
    NotFound,
}
impl Display for InfoOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Found => write!(f, "found"),
            Self::NotFound => write!(f, "not-found"),
        }
    }
}
impl InfoOutput {
    fn from_action(action: &FlatpakAction) -> Result<Self> {
        let mut command = action.get_check_command();
        let output = command
            .output()
            .context("Failed to run flatpak info command")?;

        if output.status.success() {
            Ok(Self::Found)
        } else {
            Ok(Self::NotFound)
        }
    }

    fn to_action_state(&self, action: &FlatpakAction) -> ActionState {
        match action {
            FlatpakAction::Install { .. } => match self {
                Self::Found => ActionState::Done,
                Self::NotFound => ActionState::Available,
            },
            FlatpakAction::Remove { .. } => match self {
                Self::Found => ActionState::Available,
                Self::NotFound => ActionState::Done,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FlatpakAction {
    Install {
        app: String,
        #[serde(default)]
        scope: Scope,
        #[serde(default)]
        repo: Option<Repo>,
        #[serde(default)]
        fail_allowed: Option<bool>,
    },
    Remove {
        app: String,
        #[serde(default)]
        scope: Scope,
        #[serde(default)]
        fail_allowed: Option<bool>,
    },
}
impl Display for FlatpakAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Install {
                app: app_id, scope, ..
            } => {
                write!(f, "Flatpak install {scope} app: {app_id}")
            }
            Self::Remove {
                app: app_id, scope, ..
            } => {
                write!(f, "Flatpak remove {scope} app: {app_id}")
            }
        }
    }
}
impl IsAction for FlatpakAction {
    fn get_command(&self, _user_context: &Arc<UserExecutionContext>) -> Command {
        match self {
            Self::Install {
                app: app_id,
                repo,
                scope,
                fail_allowed: _,
            } => {
                let mut command = Command::new("flatpak");
                command.arg(scope.to_arg());
                command.arg("install");
                if let Some(repo) = repo {
                    command.arg(repo.to_arg());
                }
                command.arg("--noninteractive");
                command.arg(app_id);

                command
            }

            Self::Remove {
                app: app_id, scope, ..
            } => {
                let mut command = Command::new("flatpak");
                command.arg(scope.to_arg());
                command.arg("uninstall");
                command.arg("--noninteractive");
                command.arg(app_id);

                command
            }
        }
    }

    fn needs_elevation(&self) -> bool {
        false
    }

    fn get_status(&self, _user_context: &Arc<UserExecutionContext>) -> Result<ActionState> {
        debug!(action = %self, "Running status command");

        let info_output = InfoOutput::from_action(self)?;
        let action_state = info_output.to_action_state(self);

        debug!(action = %self, info_output = %info_output, action_state = %action_state, "Action state");

        Ok(action_state)
    }

    fn fail_allowed(&self) -> bool {
        match self {
            Self::Install { fail_allowed, .. } | Self::Remove { fail_allowed, .. } => {
                fail_allowed.is_some_and(|fail_allowed| fail_allowed)
            }
        }
    }

    fn to_undo(&self) -> Self {
        match self.clone() {
            Self::Install {
                app: app_id,
                scope,
                repo: _,
                fail_allowed,
            } => Self::Remove {
                app: app_id,
                scope,
                fail_allowed,
            },

            Self::Remove {
                app: app_id,
                scope,
                fail_allowed,
            } => Self::Install {
                app: app_id,
                scope,
                repo: None,
                fail_allowed,
            },
        }
    }

    fn before_retry(&self, _output: &std::process::Output) -> Option<Command> {
        None
    }

    fn needs_reboot(&self) -> bool {
        false
    }
}
impl FlatpakAction {
    fn get_check_command(&self) -> Command {
        match self {
            Self::Install {
                app: app_id, scope, ..
            }
            | Self::Remove {
                app: app_id, scope, ..
            } => {
                let mut command = Command::new("flatpak");
                command.arg(scope.to_arg()).arg("info").arg(app_id);

                command
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    fn get_user_context() -> Arc<UserExecutionContext> {
        UserExecutionContext::new(Cli::parse_from::<[_; 0], &str>([])).unwrap()
    }

    fn install_action() -> FlatpakAction {
        FlatpakAction::Install {
            app: "org.gimp.GIMP".to_string(),
            scope: Scope::User,
            repo: Some(Repo::default()),
            fail_allowed: Some(false),
        }
    }

    fn remove_action() -> FlatpakAction {
        FlatpakAction::Remove {
            app: "org.gimp.GIMP".to_string(),
            scope: Scope::System,
            fail_allowed: Some(false),
        }
    }

    fn command_args(action: &FlatpakAction) -> Vec<String> {
        let user_context = get_user_context();

        action
            .get_command(&user_context)
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn serde_yaml_parses_install_action() {
        let yaml = r"
            type: install
            app: org.gimp.GIMP
            scope: user
            repo: fedora
            fail_allowed: false
            ";

        let action: FlatpakAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            FlatpakAction::Install {
                app: "org.gimp.GIMP".to_string(),
                scope: Scope::User,
                repo: Some(Repo::Fedora),
                fail_allowed: Some(false),
            }
        );
        assert_eq!(
            command_args(&action),
            vec![
                "--user",
                "install",
                "fedora",
                "--noninteractive",
                "org.gimp.GIMP"
            ]
        );
        assert!(!action.fail_allowed());
        assert!(!action.needs_elevation());
    }

    #[test]
    fn serde_yaml_parses_remove_action() {
        let yaml = r"
            type: remove
            app: org.gimp.GIMP
            scope: system
            fail_allowed: true
            ";

        let action: FlatpakAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            FlatpakAction::Remove {
                app: "org.gimp.GIMP".to_string(),
                scope: Scope::System,
                fail_allowed: Some(true),
            }
        );
        assert_eq!(
            command_args(&action),
            vec!["--system", "uninstall", "--noninteractive", "org.gimp.GIMP"]
        );
        assert!(action.fail_allowed());
        assert!(!action.needs_elevation());
    }

    #[test]
    fn serde_yaml_parses_optional_values() {
        let yaml = r"
            type: install
            app: org.gimp.GIMP
            ";

        let action: FlatpakAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            FlatpakAction::Install {
                app: "org.gimp.GIMP".to_string(),
                scope: Scope::default(),
                repo: None,
                fail_allowed: None,
            }
        );
        assert_eq!(
            command_args(&action),
            vec!["--system", "install", "--noninteractive", "org.gimp.GIMP"]
        );
        assert!(!action.fail_allowed());
        assert!(!action.needs_elevation());
    }

    #[test]
    fn serde_yaml_parses_explicit_flathub_repo() {
        let yaml = r"
            type: install
            app: org.gimp.GIMP
            repo: flathub
            ";

        let action: FlatpakAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            FlatpakAction::Install {
                app: "org.gimp.GIMP".to_string(),
                scope: Scope::System,
                repo: Some(Repo::Flathub),
                fail_allowed: None,
            }
        );
        assert_eq!(
            command_args(&action),
            vec![
                "--system",
                "install",
                "flathub",
                "--noninteractive",
                "org.gimp.GIMP"
            ]
        );
    }

    #[test]
    fn serde_yaml_round_trips_install_action() {
        let action = install_action();

        let yaml = serde_yaml::to_string(&action).unwrap();
        let parsed: FlatpakAction = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed, action);
    }

    #[test]
    fn serde_yaml_round_trips_install_action_without_repo() {
        let action = FlatpakAction::Install {
            app: "org.gimp.GIMP".to_string(),
            scope: Scope::User,
            repo: None,
            fail_allowed: Some(false),
        };

        let yaml = serde_yaml::to_string(&action).unwrap();
        let parsed: FlatpakAction = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed, action);
    }

    #[test]
    fn serde_yaml_round_trips_remove_action() {
        let action = remove_action();

        let yaml = serde_yaml::to_string(&action).unwrap();
        let parsed: FlatpakAction = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed, action);
    }

    #[test]
    fn install_status_maps_correctly() {
        let action = install_action();

        assert_eq!(
            InfoOutput::Found.to_action_state(&action),
            ActionState::Done
        );

        assert_eq!(
            InfoOutput::NotFound.to_action_state(&action),
            ActionState::Available
        );
    }

    #[test]
    fn remove_status_maps_correctly() {
        let action = remove_action();

        assert_eq!(
            InfoOutput::Found.to_action_state(&action),
            ActionState::Available
        );

        assert_eq!(
            InfoOutput::NotFound.to_action_state(&action),
            ActionState::Done
        );
    }

    #[test]
    fn undo_flips_install_to_remove() {
        let action = install_action();

        assert_eq!(
            action.to_undo(),
            FlatpakAction::Remove {
                app: "org.gimp.GIMP".to_string(),
                scope: Scope::User,
                fail_allowed: Some(false),
            }
        );
    }

    #[test]
    fn undo_flips_remove_to_install_without_repo() {
        let action = remove_action();

        assert_eq!(
            action.to_undo(),
            FlatpakAction::Install {
                app: "org.gimp.GIMP".to_string(),
                scope: Scope::System,
                repo: None,
                fail_allowed: Some(false),
            }
        );
    }

    #[test]
    fn before_retry_always_returns_none() {
        let action = install_action();
        let output = std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: b"error".to_vec(),
        };

        assert!(action.before_retry(&output).is_none());
    }

    #[test]
    fn scope_defaults_to_system() {
        let yaml = r"
            type: install
            app: org.gimp.GIMP
            ";

        let action: FlatpakAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            FlatpakAction::Install {
                app: "org.gimp.GIMP".to_string(),
                scope: Scope::System,
                repo: None,
                fail_allowed: None,
            }
        );
    }

    #[test]
    fn scope_defaults_to_system_on_remove() {
        let yaml = r"
            type: remove
            app: org.gimp.GIMP
            ";

        let action: FlatpakAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(
            action,
            FlatpakAction::Remove {
                app: "org.gimp.GIMP".to_string(),
                scope: Scope::System,
                fail_allowed: None,
            }
        );
    }
}
