use super::{ActionState, OutputExt};
use crate::application::task_manager::{
    actions::IsAction, user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result, bail};
use common::utils;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display},
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::Arc,
};
use tracing::{debug, error, info, warn};

#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UdevOperator {
    Match,
    NotMatch,
    Assign,
    Append,
    AssignFinal,
}
impl Display for UdevOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Match => "==",
            Self::NotMatch => "!=",
            Self::Assign => "=",
            Self::Append => "+=",
            Self::AssignFinal => ":=",
        })
    }
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq)]
pub struct UdevRuleEntry {
    pub operator: UdevOperator,
    pub value: String,
}
impl Display for UdevRuleEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\"{}\"", self.operator, self.value)
    }
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq)]
pub struct UdevRuleEntryWithSelector {
    pub selector: String,
    pub operator: UdevOperator,
    pub value: String,
}
impl Display for UdevRuleEntryWithSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{}}}{}\"{}\"",
            self.selector, self.operator, self.value
        )
    }
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UdevKey {
    Action(UdevRuleEntry),
    Devpath(UdevRuleEntry),
    Kernel(UdevRuleEntry),
    Kernels(UdevRuleEntry),
    Subsystem(UdevRuleEntry),
    Subsystems(UdevRuleEntry),
    Devtype(UdevRuleEntry),
    Driver(UdevRuleEntry),
    Drivers(UdevRuleEntry),
    Attr(UdevRuleEntryWithSelector),
    Attrs(UdevRuleEntryWithSelector),
    Sysctl(UdevRuleEntryWithSelector),
    Env(UdevRuleEntryWithSelector),
    Tag(UdevRuleEntry),
    Tags(UdevRuleEntry),
    Result(UdevRuleEntry),
    Const(UdevRuleEntryWithSelector),
    Name(UdevRuleEntry),
    Symlink(UdevRuleEntry),
    Owner(UdevRuleEntry),
    Group(UdevRuleEntry),
    Mode(UdevRuleEntry),
    StaticNode(UdevRuleEntry),
    Options(UdevRuleEntry),
    Run(UdevRuleEntry),
    Import(UdevRuleEntryWithSelector),
    Program(UdevRuleEntry),
    Test(UdevRuleEntry),
    Label(UdevRuleEntry),
    Goto(UdevRuleEntry),
}
impl Display for UdevKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Action(entry) => write!(f, "ACTION{entry}"),
            Self::Devpath(entry) => write!(f, "DEVPATH{entry}"),
            Self::Kernel(entry) => write!(f, "KERNEL{entry}"),
            Self::Kernels(entry) => write!(f, "KERNELS{entry}"),
            Self::Subsystem(entry) => write!(f, "SUBSYSTEM{entry}"),
            Self::Subsystems(entry) => write!(f, "SUBSYSTEMS{entry}"),
            Self::Devtype(entry) => write!(f, "DEVTYPE{entry}"),
            Self::Driver(entry) => write!(f, "DRIVER{entry}"),
            Self::Drivers(entry) => write!(f, "DRIVERS{entry}"),
            Self::Attr(entry) => write!(f, "ATTR{entry}"),
            Self::Attrs(entry) => write!(f, "ATTRS{entry}"),
            Self::Sysctl(entry) => write!(f, "SYSCTL{entry}"),
            Self::Env(entry) => write!(f, "ENV{entry}"),
            Self::Tag(entry) => write!(f, "TAG{entry}"),
            Self::Tags(entry) => write!(f, "TAGS{entry}"),
            Self::Result(entry) => write!(f, "RESULT{entry}"),
            Self::Const(entry) => write!(f, "CONST{entry}"),
            Self::Name(entry) => write!(f, "NAME{entry}"),
            Self::Symlink(entry) => write!(f, "SYMLINK{entry}"),
            Self::Owner(entry) => write!(f, "OWNER{entry}"),
            Self::Group(entry) => write!(f, "GROUP{entry}"),
            Self::Mode(entry) => write!(f, "MODE{entry}"),
            Self::StaticNode(entry) => write!(f, "STATIC_NODE{entry}"),
            Self::Options(entry) => write!(f, "OPTIONS{entry}"),
            Self::Run(entry) => write!(f, "RUN{entry}"),
            Self::Import(entry) => write!(f, "IMPORT{entry}"),
            Self::Program(entry) => write!(f, "PROGRAM{entry}"),
            Self::Test(entry) => write!(f, "TEST{entry}"),
            Self::Label(entry) => write!(f, "LABEL{entry}"),
            Self::Goto(entry) => write!(f, "GOTO{entry}"),
        }
    }
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UdevAction {
    Add {
        order: usize,
        name: String,
        rule: Vec<UdevKey>,
    },
    Remove {
        order: usize,
        name: String,
        rule: Vec<UdevKey>,
    },
}
impl Display for UdevAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add { order, name, .. } => {
                write!(f, "Add udev rule {order}-{name}.rules")
            }
            Self::Remove { order, name, .. } => {
                write!(f, "Remove udev rule {order}-{name}.rules")
            }
        }
    }
}
impl IsAction for UdevAction {
    fn run_action(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
        needs_user_contex: bool,
    ) -> Result<()> {
        match self {
            Self::Add { .. } => self.create_rules_file(output, user_context)?,
            Self::Remove { .. } => self.remove_rules_file(output, user_context)?,
        }

        output.append_stdout("== Reloading udev rules");

        let mut command = Self::reload_rules_command();
        if needs_user_contex {
            user_context.apply_to(&mut command);
        }

        let reload_output = command
            .output()
            .context("Failed to run udev reload rules command")?;

        let success = reload_output.status.success();
        output.append_output(reload_output);

        if !success {
            bail!("udevadm control --reload-rules failed");
        }

        Ok(())
    }

    fn needs_elevation(&self) -> bool {
        true
    }

    fn get_status(&self, user_context: &Arc<UserExecutionContext>) -> ActionState {
        debug!(action = %self, "Checking udev rule status");

        let path = self.file_path(user_context);

        match self {
            Self::Add { .. } => {
                let Ok(expected) = self.create_rule() else {
                    error!("Failed to create the udev rule");
                    return ActionState::UnAvailable;
                };

                match fs::read(&path) {
                    Ok(actual) if actual == expected.as_bytes() => match Self::verify_file(&path) {
                        Ok(()) => ActionState::Done,
                        Err(error) => {
                            warn!(
                                %error,
                                "Existing udev rule failed verification"
                            );
                            ActionState::Available
                        }
                    },
                    Ok(_) => ActionState::Available,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        ActionState::Available
                    }
                    Err(error) => {
                        error!(%error, "Failed to read udev rule file {}", path.display());
                        ActionState::UnAvailable
                    }
                }
            }

            Self::Remove { .. } => match fs::metadata(&path) {
                Ok(_) => ActionState::Available,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => ActionState::Done,
                Err(error) => {
                    error!(%error, "Failed to inspect udev rule file {}", path.display());
                    ActionState::UnAvailable
                }
            },
        }
    }

    fn fail_allowed(&self) -> bool {
        false
    }

    fn to_undo(&self) -> Self {
        match self.clone() {
            Self::Add { order, name, rule } => Self::Remove { order, name, rule },
            Self::Remove { order, name, rule } => Self::Add { order, name, rule },
        }
    }

    fn before_retry(
        &self,
        _output: &mut Output,
        _user_context: &Arc<UserExecutionContext>,
        _needs_user_contex: bool,
    ) -> Option<Result<()>> {
        None
    }

    fn needs_reboot(&self) -> bool {
        false
    }
}

impl UdevAction {
    fn file_path(&self, user_context: &Arc<UserExecutionContext>) -> PathBuf {
        match self {
            Self::Add { order, name, .. } | Self::Remove { order, name, .. } => user_context
                .system_config_dir
                .join("udev")
                .join("rules.d")
                .join(format!("{order}-{name}.rules")),
        }
    }

    fn create_rule(&self) -> Result<String> {
        let rule = match self {
            Self::Add { rule, .. } | Self::Remove { rule, .. } => rule,
        };

        if rule.is_empty() {
            bail!("A udev rule must contain at least one entry");
        }

        Ok(format!(
            "{}\n",
            rule.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    fn verify_file(path: &Path) -> Result<()> {
        debug!(path = %path.display(), "Verifying udev rule file");

        let output = Command::new("udevadm")
            .arg("verify")
            .arg(path)
            .output()
            .context("Failed to execute udevadm verify")?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            bail!(
                "udevadm verify failed for {} ({}):\n{}{}",
                path.display(),
                output.status,
                stdout,
                stderr
            );
        }

        Ok(())
    }

    fn create_rules_file(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
    ) -> Result<()> {
        debug!("Creating udev rule file");

        let path = self.file_path(user_context);
        let parent_dir = path
            .parent()
            .context("Failed to get parent of udev rule file path")?;

        output.append_stdout("== Creating rule");
        let rule = self.create_rule()?;

        fs::create_dir_all(parent_dir).context("Failed to create udev rules directory")?;

        output.append_stdout("== Writing rule file");
        fs::write(&path, rule.as_bytes())
            .with_context(|| format!("Failed to write {}", path.display()))?;

        output.append_stdout("== Setting permissions");
        if let Err(error) = fs::set_permissions(&path, fs::Permissions::from_mode(0o644)) {
            let cleanup_result = fs::remove_file(&path);

            return Err(error)
                .context("Failed to set udev rule file permissions")
                .context(format!(
                    "File: {}; cleanup result: {cleanup_result:?}",
                    path.display()
                ));
        }

        output.append_stdout("== Verify rule file");
        if let Err(error) = Self::verify_file(&path) {
            output.append_stderr("== Error verifying rule file, removing file");

            if let Err(cleanup_error) = fs::remove_file(&path) {
                output.append_stderr("== Failed to remove rule file");

                return Err(error).context(format!(
                    "Failed to remove invalid rule file {}: {cleanup_error}",
                    path.display()
                ));
            }

            return Err(error)
                .context(format!("Removed invalid udev rule file {}", path.display()));
        }

        info!(
            path = %path.display(),
            "Udev rule verified successfully"
        );
        output.append_stdout("== Udev rule verified");

        Ok(())
    }

    fn remove_rules_file(
        &self,
        output: &mut Output,
        user_context: &Arc<UserExecutionContext>,
    ) -> Result<()> {
        debug!("Removing udev rule file");
        output.append_stdout("== Removing udev rule file");

        let path = self.file_path(user_context);

        match fs::remove_file(&path) {
            Ok(()) => {
                info!(
                    path = %path.display(),
                    "Removed udev rule file"
                );
                output.append_stdout("== Removed udev rule file");

                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                output.append_stdout("== Udev rule file is already removed");
                debug!(
                    path = %path.display(),
                    "Udev rule file is already absent"
                );

                Ok(())
            }
            Err(error) => {
                output.append_stderr("== Failed to remove udev file");
                Err(error)
                    .with_context(|| format!("Failed to remove udev rule file {}", path.display()))
            }
        }
    }

    fn reload_rules_command() -> Command {
        debug!("Reloading udev rules");

        if utils::env::is_devcontainer() {
            return Command::new("true");
        }

        let mut command = Command::new("udevadm");
        command.arg("control").arg("--reload-rules");
        command
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;
    use std::path::Path;

    fn action() -> UdevAction {
        UdevAction::Add {
            order: 81,
            name: "wol".to_string(),
            rule: vec![
                UdevKey::Action(UdevRuleEntry {
                    operator: UdevOperator::Match,
                    value: "add".to_string(),
                }),
                UdevKey::Subsystem(UdevRuleEntry {
                    operator: UdevOperator::Match,
                    value: "net".to_string(),
                }),
                UdevKey::Name(UdevRuleEntry {
                    operator: UdevOperator::Match,
                    value: "en*".to_string(),
                }),
                UdevKey::Run(UdevRuleEntry {
                    operator: UdevOperator::Append,
                    value: "/usr/bin/ethtool -s $name wol g".to_string(),
                }),
            ],
        }
    }

    fn get_user_context() -> Arc<UserExecutionContext> {
        UserExecutionContext::new(Cli::parse_from::<[_; 0], &str>([]), Path::new("/etc")).unwrap()
    }

    #[test]
    fn operator_display_formats_correctly() {
        assert_eq!(UdevOperator::Match.to_string(), "==");
        assert_eq!(UdevOperator::NotMatch.to_string(), "!=");
        assert_eq!(UdevOperator::Assign.to_string(), "=");
        assert_eq!(UdevOperator::Append.to_string(), "+=");
        assert_eq!(UdevOperator::AssignFinal.to_string(), ":=");
    }

    #[test]
    fn rule_entry_display_formats_correctly() {
        let entry = UdevRuleEntry {
            operator: UdevOperator::Match,
            value: "add".to_string(),
        };

        assert_eq!(entry.to_string(), "==\"add\"");
    }

    #[test]
    fn rule_entry_with_selector_display_formats_correctly() {
        let entry = UdevRuleEntryWithSelector {
            selector: "idVendor".to_string(),
            operator: UdevOperator::Match,
            value: "1234".to_string(),
        };

        assert_eq!(entry.to_string(), "{idVendor}==\"1234\"");
    }

    #[test]
    fn udev_key_display_formats_correctly() {
        let entry = UdevRuleEntry {
            operator: UdevOperator::Match,
            value: "add".to_string(),
        };

        assert_eq!(
            UdevKey::Action(entry.clone()).to_string(),
            "ACTION==\"add\""
        );
        assert_eq!(
            UdevKey::Devpath(entry.clone()).to_string(),
            "DEVPATH==\"add\""
        );
        assert_eq!(
            UdevKey::Kernel(entry.clone()).to_string(),
            "KERNEL==\"add\""
        );
        assert_eq!(
            UdevKey::Kernels(entry.clone()).to_string(),
            "KERNELS==\"add\""
        );
        assert_eq!(
            UdevKey::Subsystem(entry.clone()).to_string(),
            "SUBSYSTEM==\"add\""
        );
        assert_eq!(
            UdevKey::Subsystems(entry.clone()).to_string(),
            "SUBSYSTEMS==\"add\""
        );
        assert_eq!(
            UdevKey::Devtype(entry.clone()).to_string(),
            "DEVTYPE==\"add\""
        );
        assert_eq!(
            UdevKey::Driver(entry.clone()).to_string(),
            "DRIVER==\"add\""
        );
        assert_eq!(
            UdevKey::Drivers(entry.clone()).to_string(),
            "DRIVERS==\"add\""
        );
        assert_eq!(UdevKey::Tag(entry.clone()).to_string(), "TAG==\"add\"");
        assert_eq!(UdevKey::Tags(entry.clone()).to_string(), "TAGS==\"add\"");
        assert_eq!(
            UdevKey::Result(entry.clone()).to_string(),
            "RESULT==\"add\""
        );
        assert_eq!(UdevKey::Name(entry.clone()).to_string(), "NAME==\"add\"");
        assert_eq!(
            UdevKey::Symlink(entry.clone()).to_string(),
            "SYMLINK==\"add\""
        );
        assert_eq!(UdevKey::Owner(entry.clone()).to_string(), "OWNER==\"add\"");
        assert_eq!(UdevKey::Group(entry.clone()).to_string(), "GROUP==\"add\"");
        assert_eq!(UdevKey::Mode(entry.clone()).to_string(), "MODE==\"add\"");
        assert_eq!(
            UdevKey::StaticNode(entry.clone()).to_string(),
            "STATIC_NODE==\"add\""
        );
        assert_eq!(
            UdevKey::Options(entry.clone()).to_string(),
            "OPTIONS==\"add\""
        );
        assert_eq!(UdevKey::Run(entry.clone()).to_string(), "RUN==\"add\"");
        assert_eq!(
            UdevKey::Program(entry.clone()).to_string(),
            "PROGRAM==\"add\""
        );
        assert_eq!(UdevKey::Test(entry.clone()).to_string(), "TEST==\"add\"");
        assert_eq!(UdevKey::Label(entry.clone()).to_string(), "LABEL==\"add\"");
        assert_eq!(UdevKey::Goto(entry).to_string(), "GOTO==\"add\"");
    }

    #[test]
    fn udev_key_selector_display_formats_correctly() {
        let entry = UdevRuleEntryWithSelector {
            selector: "idVendor".to_string(),
            operator: UdevOperator::Match,
            value: "1234".to_string(),
        };

        assert_eq!(
            UdevKey::Attr(entry.clone()).to_string(),
            "ATTR{idVendor}==\"1234\""
        );
        assert_eq!(
            UdevKey::Attrs(entry.clone()).to_string(),
            "ATTRS{idVendor}==\"1234\""
        );
        assert_eq!(
            UdevKey::Sysctl(entry.clone()).to_string(),
            "SYSCTL{idVendor}==\"1234\""
        );
        assert_eq!(
            UdevKey::Env(entry.clone()).to_string(),
            "ENV{idVendor}==\"1234\""
        );
        assert_eq!(
            UdevKey::Const(entry.clone()).to_string(),
            "CONST{idVendor}==\"1234\""
        );
        assert_eq!(
            UdevKey::Import(entry).to_string(),
            "IMPORT{idVendor}==\"1234\""
        );
    }

    #[test]
    fn create_rule_formats_wol_rule_in_order() {
        let action = action();

        assert_eq!(
            action.create_rule().unwrap(),
            concat!(
                "ACTION==\"add\", ",
                "SUBSYSTEM==\"net\", ",
                "NAME==\"en*\", ",
                "RUN+=\"/usr/bin/ethtool -s $name wol g\"\n"
            )
        );
    }

    #[test]
    fn create_rule_rejects_empty_rule() {
        let action = UdevAction::Add {
            order: 81,
            name: "wol".to_string(),
            rule: vec![],
        };

        let error = action.create_rule().unwrap_err();

        assert!(
            error
                .to_string()
                .contains("A udev rule must contain at least one entry")
        );
    }

    #[test]
    fn serde_yaml_parses_add_action() {
        let yaml = r#"
        type: add
        order: 81
        name: wol
        rule:
        - action:
            operator: match
            value: add
        - subsystem:
            operator: match
            value: net
        - name:
            operator: match
            value: "en*"
        - run:
            operator: append
            value: "/usr/bin/ethtool -s $name wol g"
        "#;

        let parsed: UdevAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(parsed, action());
        assert_eq!(
            parsed.create_rule().unwrap(),
            concat!(
                "ACTION==\"add\", ",
                "SUBSYSTEM==\"net\", ",
                "NAME==\"en*\", ",
                "RUN+=\"/usr/bin/ethtool -s $name wol g\"\n"
            )
        );
    }

    #[test]
    fn serde_yaml_parses_selector_based_key() {
        let yaml = r#"
        type: add
        order: 70
        name: usb-device
        rule:
        - attr:
            selector: idVendor
            operator: match
            value: "1234"
        "#;

        let parsed: UdevAction = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(parsed.create_rule().unwrap(), "ATTR{idVendor}==\"1234\"\n");
    }

    #[test]
    fn serde_yaml_rejects_missing_selector() {
        let yaml = r#"
        type: add
        order: 70
        name: usb-device
        rule:
        - attr:
            operator: match
            value: "1234"
        "#;

        assert!(serde_yaml::from_str::<UdevAction>(yaml).is_err());
    }

    #[test]
    fn action_display_formats_correctly() {
        assert_eq!(action().to_string(), "Add udev rule 81-wol.rules");

        let remove = UdevAction::Remove {
            order: 81,
            name: "wol".to_string(),
            rule: vec![],
        };

        assert_eq!(remove.to_string(), "Remove udev rule 81-wol.rules");
    }

    #[test]
    fn to_undo_swaps_add_and_remove() {
        let add = action();

        let remove = add.to_undo();

        assert_eq!(
            remove,
            UdevAction::Remove {
                order: 81,
                name: "wol".to_string(),
                rule: match add.clone() {
                    UdevAction::Add { rule, .. } => rule,
                    UdevAction::Remove { .. } => unreachable!(),
                },
            }
        );

        assert_eq!(remove.to_undo(), add);
    }

    #[test]
    fn action_properties_are_correct() {
        let action = action();

        assert!(action.needs_elevation());
        assert!(!action.fail_allowed());
        assert!(!action.needs_reboot());
        assert!(
            action
                .before_retry(
                    &mut Output {
                        status: std::process::ExitStatus::default(),
                        stdout: vec![],
                        stderr: vec![],
                    },
                    &get_user_context(),
                    false,
                )
                .is_none()
        );
    }

    #[test]
    fn file_path_uses_order_and_name() {
        let action = action();
        let context = get_user_context();

        assert_eq!(
            action.file_path(&context),
            context
                .system_config_dir
                .join("udev")
                .join("rules.d")
                .join("81-wol.rules")
        );
    }
}
