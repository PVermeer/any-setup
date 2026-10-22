use super::ActionState;
use crate::application::task_manager::{
    actions::{IsAction, OutputExt},
    user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result, bail};
use common::utils;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    fs,
    os::unix::{fs as fs_unix, process::ExitStatusExt},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Output},
    sync::Arc,
};
use tracing::{debug, error, info, warn};

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "lowercase")]
pub enum SymlinkAction {
    Udev(String),
    Pipewire(String),

    UndoUdev(String),
    UndoPipewire(String),
}

impl Display for SymlinkAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Udev(filename) => write!(f, "Symlink udev rule {filename}"),
            Self::Pipewire(filename) => write!(f, "Symlink PipeWire config {filename}"),

            Self::UndoUdev(filename) => write!(f, "Undo symlink udev rule {filename}"),
            Self::UndoPipewire(filename) => write!(f, "Undo symlink PipeWire config {filename}"),
        }
    }
}

impl SymlinkAction {
    fn source_path(&self, user_context: &Arc<UserExecutionContext>) -> Result<PathBuf> {
        let filename = match self {
            Self::Udev(filename)
            | Self::Pipewire(filename)
            | Self::UndoUdev(filename)
            | Self::UndoPipewire(filename) => filename,
        };
        let Some(configs_dir) = &user_context.configs_dir else {
            let message = "Failed to get source configs dir";
            error!(?user_context, message);
            bail!(message)
        };

        let path = configs_dir.join(filename);

        debug!(
            action = %self,
            source = %path.display(),
            "Resolved config source path"
        );

        Ok(path)
    }

    fn target_paths(&self, user_context: &Arc<UserExecutionContext>) -> Vec<PathBuf> {
        let paths = match self {
            Self::Udev(filename) | Self::UndoUdev(filename) => Vec::from([user_context
                .system_config_dir
                .join("udev")
                .join("rules.d")
                .join(filename)]),

            Self::Pipewire(filename) | Self::UndoPipewire(filename) => {
                let base_dir = user_context.system_config_dir.join("pipewire");
                Vec::from([
                    base_dir.join("pipewire-pulse.conf.d").join(filename),
                    base_dir.join("pipewire.conf.d").join(filename),
                ])
            }
        };

        debug!(
            action = %self,
            targets = %paths.iter().map(|path| path.display().to_string()).collect::<Vec<_>>().join(":"),
            "Resolved config target paths"
        );

        paths
    }

    fn is_our_symlink(&self, source: &PathBuf, target: &PathBuf) -> bool {
        debug!("Checking config symlink");

        match fs::read_link(target) {
            Ok(link) => {
                let resolved = if link.is_absolute() {
                    link
                } else {
                    target.parent().unwrap_or(Path::new(".")).join(link)
                };

                let matches = resolved == *source;

                debug!(
                    action = %self,
                    target = %target.display(),
                    link = %resolved.display(),
                    expected_source = %source.display(),
                    matches,
                    "Checked config symlink target"
                );

                matches
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                debug!(
                    action = %self,
                    target = %target.display(),
                    "Config symlink does not exist"
                );

                false
            }
            Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
                debug!(
                    action = %self,
                    target = %target.display(),
                    "Config target exists but is not a symlink"
                );

                false
            }
            Err(error) => {
                error!(
                    action = %self,
                    target = %target.display(),
                    %error,
                    "Failed to inspect config symlink"
                );

                false
            }
        }
    }

    fn get_validate_command(
        &self,
        source_path: &PathBuf,
        user_context: &Arc<UserExecutionContext>,
    ) -> Option<Command> {
        let mut command = match self {
            Self::Udev(_) => {
                let mut command = Command::new("udevadm");
                command.arg("verify");
                command.arg(source_path);

                Some(command)
            }

            Self::Pipewire(_) => {
                warn!("Unable to verify pipewire config files");
                None
            }

            Self::UndoPipewire(_) | Self::UndoUdev(_) => None,
        };

        if let Some(command) = command.as_mut() {
            user_context.apply_to(command);
        }

        command
    }

    fn validate_source_file(
        &self,
        source_file_path: &PathBuf,
        user_context: &Arc<UserExecutionContext>,
    ) -> Result<()> {
        let mut command = self.get_validate_command(source_file_path, user_context);

        let Some(command) = command.as_mut() else {
            return Ok(());
        };

        let Ok(output) = command.output() else {
            let message = "Failed to run validate command for source config file";
            error!(
                path = %source_file_path.display(),
                message,
            );
            bail!(message)
        };

        if !output.status.success() {
            let stdout = utils::command::parse_output(&output.stdout);
            let stderr = utils::command::parse_output(&output.stderr);

            let message = "Failed to validate source config file";

            error!(path = %source_file_path.display(), stdout, stderr, message);
            bail!(message);
        }

        Ok(())
    }

    fn match_action_state_with_target(&self, source: &PathBuf, target: &PathBuf) -> ActionState {
        match self {
            Self::Pipewire(_) | Self::Udev(_) => {
                if self.is_our_symlink(source, target) {
                    debug!(action = %self, "Target symlink already exists");
                    return ActionState::Done;
                }

                if target.is_file() || target.is_symlink() {
                    let message = "Target file already exists and it's not ours";
                    debug!(action = %self, message);
                    return ActionState::UnAvailable(message.to_string());
                }

                ActionState::Available
            }

            Self::UndoPipewire(_) | Self::UndoUdev(_) => {
                if self.is_our_symlink(source, target) {
                    debug!(action = %self, "Target symlink is ours");
                    return ActionState::Available;
                }

                if target.is_file() || target.is_symlink() {
                    let message = "Target file already exists and it's not ours";
                    debug!(action = %self, message);
                    return ActionState::UnAvailable(message.to_string());
                }

                ActionState::Done
            }
        }
    }

    fn add_symlinks(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
    ) -> Result<()> {
        debug!(action = %self, "Adding config symlink");

        let source_path = self.source_path(user_context)?;
        let target_paths = self.target_paths(user_context);

        if !source_path.is_file() {
            let message = "Config source does not exist or is not a file";
            error!(
                action = %self,
                source = %source_path.display(),
                message
            );
            output.append_stderr(message);

            bail!(
                "Config source does not exist or is not a file: {}",
                source_path.display()
            );
        }

        output.append_stdout("== Validating source file");

        match self.validate_source_file(&source_path, user_context) {
            Ok(()) => {
                output.append_stdout("Source file is valid");
            }
            Err(error) => {
                output.append_stderr(&error.to_string());
                bail!("Failed to validate source file");
            }
        }

        let mut new_symlinks = Vec::new();
        let mut error_message = "";

        for target_path in target_paths {
            if let Some(parent_dir) = target_path.parent()
                && !parent_dir.is_dir()
                && fs::create_dir_all(parent_dir).is_err()
            {
                error_message = "Failed to create config target directory";
                error!(dir = %parent_dir.display(), error_message);
                output.append_stderr(error_message);

                break;
            }

            if self.is_our_symlink(&source_path, &target_path) {
                let message = "Config symlink already exists";
                debug!(
                    action = %self,
                    target = %target_path.display(),
                    message
                );
                output.append_stdout(message);

                continue;
            }

            if target_path.is_file() || target_path.is_symlink() {
                error_message = "Symlink target already exists and it's not ours";
                error!(target = %&target_path.display(), error_message);

                output.append_stderr(error_message);
                break;
            }

            if let Err(error) = fs_unix::symlink(&source_path, &target_path) {
                error_message = "Failed to create config symlink";

                error!(
                    %error,
                    source = %source_path.display(),
                    target = %target_path.display(),
                    error_message,
                );
                output.append_stderr(error_message);
                output.append_stderr(&error.to_string());

                break;
            }

            new_symlinks.push(target_path.clone());

            debug!(
                action = %self,
                source = %source_path.display(),
                targets = %target_path.display(),
                "Successfully created config symlink"
            );

            let message = format!(
                "Created config symlink {} -> {}",
                target_path.display(),
                source_path.display()
            );

            info!(%message);
            output.append_stdout(&message);
        }

        if !error_message.is_empty() {
            let message = "Failed to create all symlinks";
            error!(error_message, message);
            output.append_stderr(message);
            output.append_stderr(error_message);

            debug!("Running cleanup");
            for symlink in new_symlinks {
                let _ = fs::remove_file(symlink);
            }

            bail!(error_message);
        }

        Ok(())
    }

    fn remove_symlinks(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
    ) -> Result<()> {
        debug!(action = %self, "Removing config symlinks");

        let source_path = self.source_path(user_context)?;
        let target_paths = self.target_paths(user_context);

        for target_path in &target_paths {
            if self.is_our_symlink(&source_path, target_path) {
                let message = "Removing config symlink";
                debug!(
                    action = %self,
                    target = %target_path.display(),
                    message
                );
                output.append_stdout(message);

                match fs::remove_file(target_path) {
                    Ok(()) => {
                        let message = "Removed symlink";
                        debug!(
                            action = %self,
                            target = %target_path.display(),
                            message
                        );
                        output.append_stdout(message);
                    }
                    Err(error) => {
                        let message = "Failed to remove symlink";
                        error!(
                            action = %self,
                            target = %target_path.display(),
                            %error,
                            message
                        );
                        output.append_stderr(message);
                    }
                }
            }
        }

        Ok(())
    }

    fn post_action(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
    ) -> Result<()> {
        debug!(action = %self, "Running post actions");

        match self {
            Self::Pipewire(_) | Self::UndoPipewire(_) => {
                output.append_stdout("Restarting pipewire");

                let mut command = Command::new("systemctl");
                command.arg("--user");
                command.arg("restart");
                command.arg("pipewire.service");
                command.arg("pipewire-pulse.service");

                user_context.apply_to(&mut command);

                let command_output = command.output().context("Failed to run systemd restart")?;

                output.append_output(command_output);

                Ok(())
            }

            Self::Udev(_) | Self::UndoUdev(_) => {
                output.append_stdout("Reloading rules");

                let mut reload_command = Command::new("udevadm");
                reload_command.arg("control");
                reload_command.arg("--reload-rules");

                user_context.apply_to(&mut reload_command);

                let reload_output = reload_command
                    .output()
                    .context("Failed to run udevadm control --reload-rules")?;

                output.append_output(reload_output);

                let mut trigger_command = Command::new("udevadm");
                trigger_command.arg("trigger");

                user_context.apply_to(&mut trigger_command);

                let trigger_output = trigger_command
                    .output()
                    .context("Failed to run udevadm control --reload-rules")?;

                output.append_output(trigger_output);

                Ok(())
            }
        }
    }
}

impl IsAction for SymlinkAction {
    fn run_action(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
        _needs_user_context: bool,
    ) -> Result<()> {
        debug!(action = %self, "Running symlink action");

        match self {
            Self::Pipewire(_) | Self::Udev(_) => self.add_symlinks(output, user_context),

            Self::UndoPipewire(_) | Self::UndoUdev(_) => self.remove_symlinks(output, user_context),
        }
        .inspect_err(|_error| {
            output.status = ExitStatus::from_raw(1);
        })?;

        self.post_action(output, user_context)
    }

    fn needs_elevation(&self) -> bool {
        true
    }

    fn get_status(&self, user_context: &Arc<UserExecutionContext>) -> ActionState {
        debug!(action = %self, "Checking symlink action status");

        if let Self::Udev(_) = self
            && !utils::command::test_command_available_sync("udevadm")
        {
            let message = "'udevadm' Command not available";
            error!(message);
            return ActionState::UnAvailable(message.to_string());
        }

        let Some(source) = self
            .source_path(user_context)
            .ok()
            .filter(|source| source.is_file())
        else {
            let message = "Failed to get source config file";
            error!(action = %self, message);
            return ActionState::UnAvailable(message.to_string());
        };

        let targets = self.target_paths(user_context);

        let action_states: Vec<ActionState> = targets
            .iter()
            .map(|target| self.match_action_state_with_target(&source, target))
            .collect();

        let final_action_state = if let Some(unavailable) = action_states
            .iter()
            .find(|state| matches!(state, ActionState::UnAvailable(_)))
        {
            unavailable.clone()
        } else if action_states
            .iter()
            .all(|state| matches!(state, ActionState::Done))
        {
            ActionState::Done
        } else if action_states
            .iter()
            .any(|state| matches!(state, ActionState::Available))
        {
            ActionState::Available
        } else {
            ActionState::UnAvailable("This should not happen".to_string())
        };

        debug!(action = %self, ?action_states, action_state = %final_action_state, "Action state for action");

        final_action_state
    }

    fn fail_allowed(&self) -> bool {
        false
    }

    fn to_undo(&self) -> Self {
        match self {
            Self::Pipewire(filename) => Self::UndoPipewire(filename.clone()),
            Self::Udev(filename) => Self::UndoUdev(filename.clone()),

            Self::UndoPipewire(filename) => Self::Pipewire(filename.clone()),
            Self::UndoUdev(filename) => Self::Udev(filename.clone()),
        }
    }

    fn before_retry(
        &self,
        _output: &mut Output,
        _user_context: &Arc<UserExecutionContext>,
        _needs_user_context: bool,
    ) -> Option<Result<()>> {
        None
    }

    fn needs_reboot(&self) -> bool {
        false
    }
}
