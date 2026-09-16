use crate::application::task_manager::actions::{Action, IsAction};
use anyhow::{Context, Result, bail};
use common::{
    dbus_query::{self, DbusConnectionType},
    utils,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    io::{BufRead, BufReader},
    os::unix::process::CommandExt,
    process::{Command, ExitStatus, Output, Stdio},
};
use tracing::debug;

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
#[derive(Serialize, Deserialize, Debug)]
pub enum ActionJsonMessage {
    ActionResults(Vec<ActionResult>),
    ActionProgress(ActionProgress),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct UserExecutionContext {
    user_id: u32,
    environment: HashMap<String, String>,
}
impl UserExecutionContext {
    fn new() -> Result<Self> {
        let environment = HashMap::from([
            (
                "XDG_RUNTIME_DIR".to_string(),
                utils::env::get_user_runtime_dir(),
            ),
            (
                "DBUS_SESSION_BUS_ADDRESS".to_string(),
                dbus_query::get_address(&DbusConnectionType::Session)?,
            ),
        ]);

        Ok(Self {
            user_id: utils::env::get_user_id(),
            environment,
        })
    }

    fn apply_to(&self, command: &mut Command) {
        command.uid(self.user_id).envs(&self.environment);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActionRunner {
    pub name: String,
    queue: Vec<Action>,
    elevate: bool,
    is_elevated: bool,
    is_undo: bool,
    user_context: UserExecutionContext,
}
impl Hash for ActionRunner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.queue.hash(state);
        self.elevate.hash(state);
        self.is_elevated.hash(state);
        self.is_undo.hash(state);
    }
}
impl ActionRunner {
    pub fn new(name: &str) -> Result<Self> {
        let user_context = UserExecutionContext::new()?;

        Ok(Self {
            name: name.to_string(),
            queue: Vec::new(),
            elevate: false,
            is_elevated: false,
            is_undo: false,
            user_context,
        })
    }

    pub fn add(&mut self, action: &Action) {
        if action.needs_elevation() {
            self.elevate = true;
        }
        self.queue.push(action.clone());
    }

    pub fn add_many(&mut self, actions: &Vec<Action>) {
        for action in actions {
            self.add(action);
        }
    }

    pub fn get_id(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    pub fn set_undo(&mut self, undo: bool) {
        self.is_undo = undo;
    }

    pub fn run(self, on_progress: Option<&dyn Fn(&ActionProgress)>) -> Result<Vec<ActionResult>> {
        let results = if self.elevate && !self.is_elevated {
            self.run_elevated(on_progress)
        } else {
            self.run_actions(on_progress)
        };

        results.context("Internal action-runner error")
    }

    fn run_actions(
        self,
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
            if self.is_elevated {
                println!(
                    "{}",
                    json!(ActionJsonMessage::ActionProgress(action_progress))
                );
            }

            let mut output = Output {
                status: ExitStatus::default(),
                stderr: Vec::new(),
                stdout: Vec::new(),
            };
            output
                .stdout
                .extend_from_slice(format!("==== Running action {} ====\n", i + 1).as_bytes());

            let mut command = action.get_command();

            if self.is_elevated && !action.needs_elevation() {
                self.user_context.apply_to(&mut command);
            }

            let command_output = command.output().context("Failed to run action command")?;

            output.stdout.extend(command_output.stdout);
            output.stderr.extend(command_output.stderr);
            output.status = command_output.status;

            if !output.status.success()
                && let Some(mut on_error_command) = action.on_error(&output)
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
                    .get_command()
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
        if self.is_elevated {
            println!(
                "{}",
                json!(ActionJsonMessage::ActionProgress(progress_finished))
            );
        }

        Ok(results)
    }

    fn run_elevated(
        mut self,
        on_progress: Option<&dyn Fn(&ActionProgress)>,
    ) -> Result<Vec<ActionResult>> {
        debug!("Running actions elevated");

        self.is_elevated = true;
        let json = json!(&self).to_string();
        self.is_elevated = false;

        let current_exe = std::env::current_exe()?;
        let mut command = "pkexec";
        if utils::env::is_devcontainer() {
            command = "sudo";
        }

        let mut command = Command::new(command);
        command
            .arg(current_exe)
            .arg("action-runner")
            .arg("--json")
            .arg(json);

        if let Some(on_progress) = on_progress {
            let mut piped = command
                .stdout(Stdio::piped())
                .spawn()
                .context("Failed to run command")?;

            let stdout = piped.stdout.take().context("Failed to capture stdout")?;
            let out_reader = BufReader::new(stdout);

            let mut action_results = None;

            for line in out_reader.lines() {
                let line = line.context("Failed to read line from stdout buffer")?;
                let json_parsed: ActionJsonMessage = serde_json::from_str(&line)?;

                match json_parsed {
                    ActionJsonMessage::ActionProgress(progress) => {
                        on_progress(&progress);
                    }
                    ActionJsonMessage::ActionResults(results) => {
                        action_results = Some(results);
                    }
                }
            }

            let Some(action_results) = action_results else {
                bail!("No results recieved from elevated action-runner");
            };

            return Ok(action_results);
        }

        let output = command
            .stdout(Stdio::piped())
            .output()
            .context("Failed to run command")?;

        let action_results: Vec<ActionResult> = serde_json::from_slice(&output.stdout)?;

        Ok(action_results)
    }
}
