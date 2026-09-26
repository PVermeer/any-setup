use super::user_execution_context::UserExecutionContext;
use crate::application::{
    pages::page_config::PageYaml,
    task_manager::actions::{Action, IsAction},
};
use anyhow::{Context, Result, bail};
use common::{app_dirs::AppDirs, utils};
use serde::{Deserialize, Serialize};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    process::{ExitStatus, Output},
    sync::Arc,
};
use tracing::{debug, error};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActionResult {
    pub action: Action,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}
impl ActionResult {
    pub fn from_output(action: &Action, output: &Output) -> Self {
        Self {
            action: action.clone(),
            success: output.status.success(),
            stdout: utils::command::parse_output(&output.stdout),
            stderr: utils::command::parse_output(&output.stderr),
        }
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ActionStatus {
    Running,
    Finished,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActionProgress {
    pub action: Option<String>,
    pub action_nr: Option<i32>,
    pub total_actions: i32,
    pub progress: f64,
    pub status: ActionStatus,
}

#[derive(Clone, Debug)]
pub struct ActionRunner {
    pub name: String,
    queue: Vec<Action>,
    elevate: bool,
    is_undo: bool,
    user_context: UserExecutionContext,
}
impl Hash for ActionRunner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.queue.hash(state);
        self.elevate.hash(state);
        self.is_undo.hash(state);
    }
}
impl ActionRunner {
    pub fn new(name: &str, actions: &[Action], user_context: &UserExecutionContext) -> Arc<Self> {
        Arc::new(Self {
            name: name.to_string(),
            queue: actions.to_vec(),
            elevate: actions.iter().any(IsAction::needs_elevation),
            is_undo: false,
            user_context: user_context.clone(),
        })
    }

    pub fn from_id(
        action_runner_id: u64,
        user_context: &UserExecutionContext,
    ) -> Result<Arc<Self>> {
        let app_dirs = AppDirs::new()?;

        if let Some(pages_dir) = &app_dirs.system_data_pages_dir
            && let Ok(pages_dir_entries) = utils::files::get_entries_in_dir(pages_dir)
        {
            for dir_entry in pages_dir_entries {
                let path = dir_entry.path();

                if path
                    .extension()
                    .is_none_or(|extension| extension != "yml" && extension != "yaml")
                {
                    continue;
                }

                let page_yaml = match PageYaml::from_file(&path) {
                    Ok(page_yaml) => page_yaml,
                    Err(error) => {
                        error!(?error);
                        continue;
                    }
                };

                let action_runners = match page_yaml
                    .into_action_runners(user_context)
                    .context("Failed to create ActionRunners from yaml")
                {
                    Ok(action_runners) => action_runners,
                    Err(error) => {
                        error!(?error);
                        continue;
                    }
                };

                if let Some(action_runner) = action_runners.get(&action_runner_id) {
                    return Ok(action_runner.clone());
                }
            }
        }

        bail!("Failed to find the ActionRunner")
    }

    pub fn get_id(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    pub fn to_undo(&self) -> Arc<Self> {
        let mut self_clone = self.clone();
        self_clone.queue = self_clone.queue.iter().map(IsAction::to_undo).collect();

        self_clone.is_undo = true;

        Arc::new(self_clone)
    }

    pub fn needs_elevation(&self) -> bool {
        self.elevate
    }

    pub fn run_actions(
        &self,
        on_progress: Option<&dyn Fn(&ActionProgress)>,
    ) -> Result<Vec<ActionResult>> {
        let mut results = Vec::new();
        let queue_length = self.queue.len();
        let queue_factor = 1.0 / queue_length as f64;
        let mut progress = 0.0;

        for (i, action) in self.queue.iter().enumerate() {
            debug!(action = action.to_string(), "Running action");

            let action_progress = ActionProgress {
                progress,
                action: Some(action.to_string()),
                action_nr: Some((i + 1).try_into()?),
                total_actions: queue_length.try_into()?,
                status: ActionStatus::Running,
            };
            if let Some(on_progress) = &on_progress {
                on_progress(&action_progress);
            }

            let mut output = Output {
                status: ExitStatus::default(),
                stderr: Vec::new(),
                stdout: Vec::new(),
            };
            output
                .stdout
                .extend_from_slice(format!("==== Running action {} ====\n", i + 1).as_bytes());

            let mut command = action.get_command(&self.user_context);

            if self.needs_elevation() && !action.needs_elevation() {
                self.user_context.apply_to(&mut command);
            }

            let command_output = command.output().context("Failed to run action command")?;

            output.stdout.extend(command_output.stdout);
            output.stderr.extend(command_output.stderr);
            output.status = command_output.status;

            if !output.status.success()
                && let Some(mut on_error_command) = action.before_retry(&output)
            {
                output
                    .stdout
                    .extend_from_slice("\n== Running on_error command\n".as_bytes());

                let on_error_output = on_error_command
                    .output()
                    .context("Failed to run on_error command")?;

                output.stdout.extend(on_error_output.stdout);
                output.stderr.extend(on_error_output.stderr);

                output
                    .stdout
                    .extend_from_slice("\n== Retrying action command\n".as_bytes());

                let retry_output = action
                    .get_command(&self.user_context)
                    .output()
                    .context("Failed to re-run action command")?;

                output.stdout.extend(retry_output.stdout);
                output.stderr.extend(retry_output.stderr);
                output.status = retry_output.status;
            }

            let action_result = ActionResult::from_output(action, &output);

            progress = (i + 1) as f64 * queue_factor;
            results.push(action_result);

            if !action.fail_allowed() && !output.status.success() {
                break;
            }
        }

        let progress_finished = ActionProgress {
            progress: 1.0,
            action: None,
            action_nr: None,
            total_actions: queue_length.try_into()?,
            status: ActionStatus::Finished,
        };
        if let Some(on_progress) = &on_progress {
            on_progress(&progress_finished);
        }

        Ok(results)
    }
}
