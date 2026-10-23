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
    RpmRepo(String),

    UndoUdev(String),
    UndoPipewire(String),
    UndoRpmRepo(String),
}

impl Display for SymlinkAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Udev(filename) => write!(f, "Symlink udev rule {filename}"),
            Self::Pipewire(filename) => write!(f, "Symlink PipeWire config {filename}"),
            Self::RpmRepo(filename) => write!(f, "Symlink rpm repo config {filename}"),

            Self::UndoUdev(filename) => write!(f, "Undo symlink udev rule {filename}"),
            Self::UndoPipewire(filename) => write!(f, "Undo symlink PipeWire config {filename}"),
            Self::UndoRpmRepo(filename) => write!(f, "Undo symlink rpm repo config {filename}"),
        }
    }
}

impl SymlinkAction {
    fn source_path(&self, user_context: &Arc<UserExecutionContext>) -> Result<PathBuf> {
        let filename = match self {
            Self::Udev(filename)
            | Self::Pipewire(filename)
            | Self::RpmRepo(filename)
            | Self::UndoUdev(filename)
            | Self::UndoPipewire(filename)
            | Self::UndoRpmRepo(filename) => filename,
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

            Self::RpmRepo(filename) | Self::UndoRpmRepo(filename) => {
                let base_dir = &user_context.system_config_dir;
                Vec::from([base_dir.join("yum.repos.d").join(filename)])
            }
        };

        debug!(
            action = %self,
            targets = %paths.iter().map(|path| path.display().to_string()).collect::<Vec<_>>().join(":"),
            "Resolved config target paths"
        );

        paths
    }

    fn is_our_symlink(&self, source: &Path, target: &PathBuf) -> bool {
        debug!("Checking config symlink");

        match fs::read_link(target) {
            Ok(link) => {
                let resolved = if link.is_absolute() {
                    link
                } else {
                    target.parent().unwrap_or(Path::new(".")).join(link)
                };

                let matches = resolved
                    .canonicalize()
                    .and_then(|resolved| source.canonicalize().map(|source| resolved == source))
                    .unwrap_or(false);

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

            Self::RpmRepo(_) => {
                warn!("Unable to verify rpm repo config files");
                None
            }

            Self::UndoPipewire(_) | Self::UndoUdev(_) | Self::UndoRpmRepo(_) => None,
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

    fn match_action_state_with_target(&self, source: &Path, target: &PathBuf) -> ActionState {
        match self {
            Self::Pipewire(_) | Self::Udev(_) | Self::RpmRepo(_) => {
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

            Self::UndoPipewire(_) | Self::UndoUdev(_) | Self::UndoRpmRepo(_) => {
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
            Self::RpmRepo(_) | Self::UndoRpmRepo(_) => {
                output.append_stdout("No post action needed");
                Ok(())
            }

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

                let reload_output = reload_command
                    .output()
                    .context("Failed to run udevadm control --reload-rules")?;

                output.append_output(reload_output);

                let mut trigger_command = Command::new("udevadm");
                trigger_command.arg("trigger");

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
            Self::Pipewire(_) | Self::Udev(_) | Self::RpmRepo(_) => {
                self.add_symlinks(output, user_context)
            }

            Self::UndoPipewire(_) | Self::UndoUdev(_) | Self::UndoRpmRepo(_) => {
                self.remove_symlinks(output, user_context)
            }
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

        match self {
            Self::Udev(_) | Self::UndoUdev(_) => {
                if !utils::command::test_command_available_sync("udevadm") {
                    let message = "'udevadm' Command not available";
                    error!(message);
                    return ActionState::UnAvailable(message.to_string());
                }
            }

            Self::Pipewire(_) | Self::UndoPipewire(_) | Self::RpmRepo(_) | Self::UndoRpmRepo(_) => {
            }
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
            Self::RpmRepo(filename) => Self::UndoRpmRepo(filename.clone()),

            Self::UndoPipewire(filename) => Self::Pipewire(filename.clone()),
            Self::UndoUdev(filename) => Self::Udev(filename.clone()),
            Self::UndoRpmRepo(filename) => Self::RpmRepo(filename.clone()),
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::symlink,
        path::Path,
        process::{ExitStatus, Output},
        sync::Arc,
    };
    use tempfile::TempDir;

    const FILENAME: &str = "test.conf";

    struct TestContext {
        _temp_dir: TempDir,
        context: Arc<UserExecutionContext>,
        configs_dir: PathBuf,
        source: PathBuf,
        udev_target: PathBuf,
        pipewire_pulse_target: PathBuf,
        pipewire_target: PathBuf,
        rpm_repo_target: PathBuf,
    }

    impl TestContext {
        fn new() -> Self {
            let temp_dir = tempfile::tempdir().unwrap();
            let configs_dir = temp_dir.path().join("configs");
            let system_config_dir = temp_dir.path().join("system");

            fs::create_dir_all(&configs_dir).unwrap();
            fs::create_dir_all(&system_config_dir).unwrap();

            let source = configs_dir.join(FILENAME);
            fs::write(&source, "test configuration\n").unwrap();

            let udev_target = system_config_dir
                .join("udev")
                .join("rules.d")
                .join(FILENAME);

            let pipewire_pulse_target = system_config_dir
                .join("pipewire")
                .join("pipewire-pulse.conf.d")
                .join(FILENAME);

            let pipewire_target = system_config_dir
                .join("pipewire")
                .join("pipewire.conf.d")
                .join(FILENAME);

            let rpm_repo_target = system_config_dir.join("yum.repos.d").join(FILENAME);

            // Adapt this initializer if UserExecutionContext requires
            // additional fields or has a dedicated test constructor.
            let context = Arc::new(UserExecutionContext {
                configs_dir: Some(configs_dir.clone()),
                system_config_dir: system_config_dir.clone(),
                ..Default::default()
            });

            Self {
                _temp_dir: temp_dir,
                context,
                configs_dir,
                source,
                udev_target,
                pipewire_pulse_target,
                pipewire_target,
                rpm_repo_target,
            }
        }

        fn output() -> Output {
            Output {
                status: ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            }
        }

        fn symlink_to_source(&self, target: &Path) {
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            symlink(&self.source, target).unwrap();
        }

        fn assert_symlink_points_to(&self, target: &Path, expected: &Path) {
            let link = fs::read_link(target).unwrap();
            let resolved = if link.is_absolute() {
                link
            } else {
                target.parent().unwrap().join(link)
            };

            assert_eq!(resolved, expected);
        }
    }

    // Display

    #[test]
    fn display_formats_all_variants() {
        assert_eq!(
            SymlinkAction::Udev(FILENAME.into()).to_string(),
            format!("Symlink udev rule {FILENAME}")
        );
        assert_eq!(
            SymlinkAction::Pipewire(FILENAME.into()).to_string(),
            format!("Symlink PipeWire config {FILENAME}")
        );
        assert_eq!(
            SymlinkAction::UndoUdev(FILENAME.into()).to_string(),
            format!("Undo symlink udev rule {FILENAME}")
        );
        assert_eq!(
            SymlinkAction::UndoPipewire(FILENAME.into()).to_string(),
            format!("Undo symlink PipeWire config {FILENAME}")
        );
        assert_eq!(
            SymlinkAction::RpmRepo(FILENAME.into()).to_string(),
            format!("Symlink rpm repo config {FILENAME}")
        );
        assert_eq!(
            SymlinkAction::UndoRpmRepo(FILENAME.into()).to_string(),
            format!("Undo symlink rpm repo config {FILENAME}")
        );
    }

    // Source and target paths

    #[test]
    fn source_path_resolves_for_all_variants() {
        let fixture = TestContext::new();

        for action in [
            SymlinkAction::Udev(FILENAME.into()),
            SymlinkAction::Pipewire(FILENAME.into()),
            SymlinkAction::UndoUdev(FILENAME.into()),
            SymlinkAction::UndoPipewire(FILENAME.into()),
            SymlinkAction::RpmRepo(FILENAME.into()),
            SymlinkAction::UndoRpmRepo(FILENAME.into()),
        ] {
            assert_eq!(
                action.source_path(&fixture.context).unwrap(),
                fixture.source
            );
        }
    }

    #[test]
    fn target_paths_resolve_for_udev_variants() {
        let fixture = TestContext::new();

        for action in [
            SymlinkAction::Udev(FILENAME.into()),
            SymlinkAction::UndoUdev(FILENAME.into()),
        ] {
            assert_eq!(
                action.target_paths(&fixture.context),
                vec![fixture.udev_target.clone()]
            );
        }
    }

    #[test]
    fn target_paths_resolve_for_pipewire_variants() {
        let fixture = TestContext::new();

        for action in [
            SymlinkAction::Pipewire(FILENAME.into()),
            SymlinkAction::UndoPipewire(FILENAME.into()),
        ] {
            assert_eq!(
                action.target_paths(&fixture.context),
                vec![
                    fixture.pipewire_pulse_target.clone(),
                    fixture.pipewire_target.clone(),
                ]
            );
        }
    }

    #[test]
    fn target_paths_resolve_for_rpm_repo_variants() {
        let fixture = TestContext::new();

        for action in [
            SymlinkAction::RpmRepo(FILENAME.into()),
            SymlinkAction::UndoRpmRepo(FILENAME.into()),
        ] {
            assert_eq!(
                action.target_paths(&fixture.context),
                vec![fixture.rpm_repo_target.clone()]
            );
        }
    }

    // Symlink detection

    #[test]
    fn is_our_symlink_recognizes_absolute_link() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());

        fixture.symlink_to_source(&fixture.pipewire_pulse_target);

        assert!(action.is_our_symlink(&fixture.source, &fixture.pipewire_pulse_target));
    }

    #[test]
    fn is_our_symlink_recognizes_relative_link() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());

        let target_parent = fixture.pipewire_pulse_target.parent().unwrap();
        fs::create_dir_all(target_parent).unwrap();

        // The target is located at:
        // <temp>/system/pipewire/pipewire-pulse.conf.d/test.conf
        //
        // The source is located at:
        // <temp>/configs/test.conf
        //
        // Three parent-directory traversals reach <temp>.
        let relative_source = PathBuf::from("../../../configs/test.conf");

        symlink(&relative_source, &fixture.pipewire_pulse_target).unwrap();

        assert_eq!(
            fs::read_link(&fixture.pipewire_pulse_target).unwrap(),
            relative_source
        );

        // Confirm that the relative link resolves to the expected source.
        let resolved = target_parent.join(&relative_source);
        assert_eq!(
            fs::canonicalize(&resolved).unwrap(),
            fs::canonicalize(&fixture.source).unwrap()
        );

        assert!(action.is_our_symlink(&fixture.source, &fixture.pipewire_pulse_target));
    }

    #[test]
    fn is_our_symlink_returns_false_for_missing_target() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());

        assert!(!action.is_our_symlink(&fixture.source, &fixture.pipewire_target));
    }

    #[test]
    fn is_our_symlink_returns_false_for_regular_file() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());

        fs::create_dir_all(fixture.pipewire_target.parent().unwrap()).unwrap();
        fs::write(&fixture.pipewire_target, "existing file").unwrap();

        assert!(!action.is_our_symlink(&fixture.source, &fixture.pipewire_target));
    }

    #[test]
    fn is_our_symlink_returns_false_for_foreign_symlink() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());

        let other_source = fixture.configs_dir.join("other.conf");
        fs::write(&other_source, "other configuration").unwrap();

        fs::create_dir_all(fixture.pipewire_target.parent().unwrap()).unwrap();
        symlink(&other_source, &fixture.pipewire_target).unwrap();

        assert!(!action.is_our_symlink(&fixture.source, &fixture.pipewire_target));
    }

    // Action status

    #[test]
    fn add_action_is_available_when_target_is_missing() {
        let fixture = TestContext::new();

        for action in [
            SymlinkAction::Pipewire(FILENAME.into()),
            SymlinkAction::Udev(FILENAME.into()),
            SymlinkAction::RpmRepo(FILENAME.into()),
        ] {
            let target = action.target_paths(&fixture.context)[0].clone();

            assert_eq!(
                action.match_action_state_with_target(&fixture.source, &target),
                ActionState::Available
            );
        }
    }

    #[test]
    fn add_action_is_done_when_target_is_our_symlink() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());

        fixture.symlink_to_source(&fixture.pipewire_pulse_target);

        assert_eq!(
            action.match_action_state_with_target(&fixture.source, &fixture.pipewire_pulse_target),
            ActionState::Done
        );
    }

    #[test]
    fn add_action_is_unavailable_when_target_is_foreign_file() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Udev(FILENAME.into());

        fs::create_dir_all(fixture.udev_target.parent().unwrap()).unwrap();
        fs::write(&fixture.udev_target, "foreign rule").unwrap();

        assert!(matches!(
            action.match_action_state_with_target(&fixture.source, &fixture.udev_target),
            ActionState::UnAvailable(_)
        ));
    }

    #[test]
    fn undo_action_is_done_when_target_is_missing() {
        let fixture = TestContext::new();

        for action in [
            SymlinkAction::UndoPipewire(FILENAME.into()),
            SymlinkAction::UndoUdev(FILENAME.into()),
            SymlinkAction::UndoRpmRepo(FILENAME.into()),
        ] {
            let target = action.target_paths(&fixture.context)[0].clone();

            assert_eq!(
                action.match_action_state_with_target(&fixture.source, &target),
                ActionState::Done
            );
        }
    }

    #[test]
    fn undo_action_is_available_when_target_is_our_symlink() {
        let fixture = TestContext::new();
        let action = SymlinkAction::UndoUdev(FILENAME.into());

        fixture.symlink_to_source(&fixture.udev_target);

        assert_eq!(
            action.match_action_state_with_target(&fixture.source, &fixture.udev_target),
            ActionState::Available
        );
    }

    #[test]
    fn undo_action_is_unavailable_when_target_is_foreign_file() {
        let fixture = TestContext::new();
        let action = SymlinkAction::UndoUdev(FILENAME.into());

        fs::create_dir_all(fixture.udev_target.parent().unwrap()).unwrap();
        fs::write(&fixture.udev_target, "foreign rule").unwrap();

        assert!(matches!(
            action.match_action_state_with_target(&fixture.source, &fixture.udev_target),
            ActionState::UnAvailable(_)
        ));
    }

    #[test]
    fn add_rpm_repo_action_is_unavailable_when_target_is_foreign_file() {
        let fixture = TestContext::new();
        let action = SymlinkAction::RpmRepo(FILENAME.into());

        fs::create_dir_all(fixture.rpm_repo_target.parent().unwrap()).unwrap();
        fs::write(&fixture.rpm_repo_target, "foreign repo").unwrap();

        assert!(matches!(
            action.match_action_state_with_target(&fixture.source, &fixture.rpm_repo_target),
            ActionState::UnAvailable(_)
        ));
    }

    #[test]
    fn undo_rpm_repo_action_is_available_when_target_is_our_symlink() {
        let fixture = TestContext::new();
        let action = SymlinkAction::UndoRpmRepo(FILENAME.into());

        fixture.symlink_to_source(&fixture.rpm_repo_target);

        assert_eq!(
            action.match_action_state_with_target(&fixture.source, &fixture.rpm_repo_target),
            ActionState::Available
        );
    }

    // Creating symlinks

    #[test]
    fn add_pipewire_symlinks_creates_both_targets() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());
        let mut output = TestContext::output();

        action.add_symlinks(&mut output, &fixture.context).unwrap();

        fixture.assert_symlink_points_to(&fixture.pipewire_pulse_target, &fixture.source);
        fixture.assert_symlink_points_to(&fixture.pipewire_target, &fixture.source);
    }

    #[test]
    fn add_pipewire_symlinks_is_idempotent() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());

        fixture.symlink_to_source(&fixture.pipewire_pulse_target);
        fixture.symlink_to_source(&fixture.pipewire_target);

        let mut output = TestContext::output();

        action.add_symlinks(&mut output, &fixture.context).unwrap();

        fixture.assert_symlink_points_to(&fixture.pipewire_pulse_target, &fixture.source);
        fixture.assert_symlink_points_to(&fixture.pipewire_target, &fixture.source);
    }

    #[test]
    fn add_pipewire_symlinks_fails_without_overwriting_foreign_file() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());

        fs::create_dir_all(fixture.pipewire_target.parent().unwrap()).unwrap();
        fs::write(&fixture.pipewire_target, "keep me").unwrap();

        let mut output = TestContext::output();

        assert!(action.add_symlinks(&mut output, &fixture.context).is_err());

        // The first link is rolled back when the second target conflicts.
        assert!(!fixture.pipewire_pulse_target.exists());
        assert_eq!(
            fs::read_to_string(&fixture.pipewire_target).unwrap(),
            "keep me"
        );
    }

    #[test]
    fn add_pipewire_symlinks_rolls_back_when_second_target_is_directory() {
        let fixture = TestContext::new();
        let action = SymlinkAction::Pipewire(FILENAME.into());

        fs::create_dir_all(fixture.pipewire_target.parent().unwrap()).unwrap();
        fs::create_dir(&fixture.pipewire_target).unwrap();

        let mut output = TestContext::output();

        assert!(action.add_symlinks(&mut output, &fixture.context).is_err());

        assert!(!fixture.pipewire_pulse_target.exists());
        assert!(fixture.pipewire_target.is_dir());
    }

    #[test]
    fn add_symlinks_fails_when_source_is_missing() {
        let fixture = TestContext::new();
        fs::remove_file(&fixture.source).unwrap();

        let action = SymlinkAction::Pipewire(FILENAME.into());
        let mut output = TestContext::output();

        assert!(action.add_symlinks(&mut output, &fixture.context).is_err());

        assert_ne!(output.stderr, [] as [u8; 0]);
        assert!(!fixture.pipewire_pulse_target.exists());
        assert!(!fixture.pipewire_target.exists());
    }

    #[test]
    fn add_rpm_repo_symlink_creates_target() {
        let fixture = TestContext::new();
        let action = SymlinkAction::RpmRepo(FILENAME.into());
        let mut output = TestContext::output();

        action.add_symlinks(&mut output, &fixture.context).unwrap();

        fixture.assert_symlink_points_to(&fixture.rpm_repo_target, &fixture.source);
    }

    #[test]
    fn add_rpm_repo_symlink_is_idempotent() {
        let fixture = TestContext::new();
        let action = SymlinkAction::RpmRepo(FILENAME.into());

        fixture.symlink_to_source(&fixture.rpm_repo_target);

        let mut output = TestContext::output();

        action.add_symlinks(&mut output, &fixture.context).unwrap();

        fixture.assert_symlink_points_to(&fixture.rpm_repo_target, &fixture.source);
    }

    // Removing symlinks

    #[test]
    fn remove_pipewire_symlinks_removes_our_links_only() {
        let fixture = TestContext::new();
        let action = SymlinkAction::UndoPipewire(FILENAME.into());

        fixture.symlink_to_source(&fixture.pipewire_pulse_target);

        let other_source = fixture.configs_dir.join("other.conf");
        fs::write(&other_source, "other configuration").unwrap();

        fs::create_dir_all(fixture.pipewire_target.parent().unwrap()).unwrap();
        symlink(&other_source, &fixture.pipewire_target).unwrap();

        let mut output = TestContext::output();

        action
            .remove_symlinks(&mut output, &fixture.context)
            .unwrap();

        assert!(!fixture.pipewire_pulse_target.is_symlink());
        assert!(fixture.pipewire_target.is_symlink());
        assert_eq!(
            fs::read_link(&fixture.pipewire_target).unwrap(),
            other_source
        );
    }

    #[test]
    fn remove_symlinks_succeeds_when_targets_are_missing() {
        let fixture = TestContext::new();
        let action = SymlinkAction::UndoPipewire(FILENAME.into());
        let mut output = TestContext::output();

        action
            .remove_symlinks(&mut output, &fixture.context)
            .unwrap();

        assert!(!fixture.pipewire_pulse_target.exists());
        assert!(!fixture.pipewire_target.exists());
    }

    #[test]
    fn remove_rpm_repo_symlink_removes_our_link_only() {
        let fixture = TestContext::new();
        let action = SymlinkAction::UndoRpmRepo(FILENAME.into());

        fixture.symlink_to_source(&fixture.rpm_repo_target);

        let mut output = TestContext::output();

        action
            .remove_symlinks(&mut output, &fixture.context)
            .unwrap();

        assert!(!fixture.rpm_repo_target.is_symlink());
    }

    // Undo mapping

    #[test]
    fn to_undo_maps_each_variant() {
        assert_eq!(
            SymlinkAction::Udev(FILENAME.into()).to_undo(),
            SymlinkAction::UndoUdev(FILENAME.into())
        );
        assert_eq!(
            SymlinkAction::Pipewire(FILENAME.into()).to_undo(),
            SymlinkAction::UndoPipewire(FILENAME.into())
        );
        assert_eq!(
            SymlinkAction::UndoUdev(FILENAME.into()).to_undo(),
            SymlinkAction::Udev(FILENAME.into())
        );
        assert_eq!(
            SymlinkAction::UndoPipewire(FILENAME.into()).to_undo(),
            SymlinkAction::Pipewire(FILENAME.into())
        );
        assert_eq!(
            SymlinkAction::RpmRepo(FILENAME.into()).to_undo(),
            SymlinkAction::UndoRpmRepo(FILENAME.into())
        );
        assert_eq!(
            SymlinkAction::UndoRpmRepo(FILENAME.into()).to_undo(),
            SymlinkAction::RpmRepo(FILENAME.into())
        );
    }

    #[test]
    fn to_undo_is_involutive() {
        for action in [
            SymlinkAction::Udev(FILENAME.into()),
            SymlinkAction::Pipewire(FILENAME.into()),
            SymlinkAction::UndoUdev(FILENAME.into()),
            SymlinkAction::UndoPipewire(FILENAME.into()),
            SymlinkAction::RpmRepo(FILENAME.into()),
            SymlinkAction::UndoRpmRepo(FILENAME.into()),
        ] {
            assert_eq!(action.to_undo().to_undo(), action);
        }
    }

    // Validation commands

    #[test]
    fn get_validate_command_is_defined_only_for_add_udev() {
        let fixture = TestContext::new();

        let command = SymlinkAction::Udev(FILENAME.into())
            .get_validate_command(&fixture.source, &fixture.context)
            .unwrap();

        assert_eq!(command.get_program(), "udevadm");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![std::ffi::OsStr::new("verify"), fixture.source.as_os_str(),]
        );

        assert!(
            SymlinkAction::Pipewire(FILENAME.into())
                .get_validate_command(&fixture.source, &fixture.context)
                .is_none()
        );
        assert!(
            SymlinkAction::UndoUdev(FILENAME.into())
                .get_validate_command(&fixture.source, &fixture.context)
                .is_none()
        );
        assert!(
            SymlinkAction::UndoPipewire(FILENAME.into())
                .get_validate_command(&fixture.source, &fixture.context)
                .is_none()
        );
        assert!(
            SymlinkAction::RpmRepo(FILENAME.into())
                .get_validate_command(&fixture.source, &fixture.context)
                .is_none()
        );
        assert!(
            SymlinkAction::UndoRpmRepo(FILENAME.into())
                .get_validate_command(&fixture.source, &fixture.context)
                .is_none()
        );
    }

    #[test]
    fn validate_source_file_skips_validation_for_pipewire_and_undo() {
        let fixture = TestContext::new();

        for action in [
            SymlinkAction::Pipewire(FILENAME.into()),
            SymlinkAction::UndoUdev(FILENAME.into()),
            SymlinkAction::UndoPipewire(FILENAME.into()),
            SymlinkAction::RpmRepo(FILENAME.into()),
            SymlinkAction::UndoRpmRepo(FILENAME.into()),
        ] {
            action
                .validate_source_file(&fixture.source, &fixture.context)
                .unwrap();
        }
    }

    // IsAction trait behavior

    #[test]
    fn action_traits_have_expected_values() {
        let fixture = TestContext::new();

        for action in [
            SymlinkAction::Udev(FILENAME.into()),
            SymlinkAction::Pipewire(FILENAME.into()),
            SymlinkAction::UndoUdev(FILENAME.into()),
            SymlinkAction::UndoPipewire(FILENAME.into()),
            SymlinkAction::RpmRepo(FILENAME.into()),
            SymlinkAction::UndoRpmRepo(FILENAME.into()),
        ] {
            assert!(action.needs_elevation());
            assert!(!action.fail_allowed());
            assert!(!action.needs_reboot());

            let mut output = TestContext::output();
            assert!(
                action
                    .before_retry(&mut output, &fixture.context, false)
                    .is_none()
            );
        }
    }

    // Serialization

    #[test]
    fn serde_round_trips_all_variants() {
        for action in [
            SymlinkAction::Udev(FILENAME.into()),
            SymlinkAction::Pipewire(FILENAME.into()),
            SymlinkAction::UndoUdev(FILENAME.into()),
            SymlinkAction::UndoPipewire(FILENAME.into()),
            SymlinkAction::RpmRepo(FILENAME.into()),
            SymlinkAction::UndoRpmRepo(FILENAME.into()),
        ] {
            let serialized = serde_json::to_string(&action).unwrap();
            let deserialized: SymlinkAction = serde_json::from_str(&serialized).unwrap();

            assert_eq!(deserialized, action);
        }
    }
}
