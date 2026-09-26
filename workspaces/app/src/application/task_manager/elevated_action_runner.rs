use super::action_runner::ActionRunnerResult;
use crate::application::task_manager::{
    action_runner::{ActionRunner, ActionStatus},
    user_execution_context::UserExecutionContext,
};
use anyhow::{Context, Result, bail};
use common::utils;
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::Arc,
};
use tracing::{debug, error};

#[derive(Debug, Serialize, Deserialize)]
pub enum ProcessRequest {
    Run {
        action_runner_id: u64,
        user_context: UserExecutionContext,
    },
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ProcessResponse {
    Ready,

    Started,

    Progress {
        action: Option<String>,
        action_nr: Option<i32>,
        total_actions: i32,
        progress: f64,
        status: ActionStatus,
    },

    Finished {
        results: ActionRunnerResult,
    },

    Failed {
        error: String,
    },

    NonJson,
}

pub struct ElevatedActionRunner {
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}
impl ElevatedActionRunner {
    pub fn start(user_context: &UserExecutionContext) -> Result<Self> {
        let current_exe = std::env::current_exe()
            .context("Failed to determine current executable for elevated ActionRunner")?;
        let subcommand = crate::cli::AppCommand::ActionRunner.to_string();

        debug!(cwd = ?glib::current_dir());

        debug!(
            ?current_exe,
            %subcommand,
            ?user_context,
            "Starting elevated ActionRunner"
        );

        let mut command = if utils::env::is_devcontainer() {
            Command::new("sudo")
        } else {
            let mut command = Command::new("pkexec");
            command.arg("--keep-cwd");
            command
        };
        user_context.apply_to(&mut command);
        command.arg(current_exe);
        command.arg(subcommand);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut process = command
            .spawn()
            .context("Failed to start elevated ActionRunner runner")?;

        let stdin = process
            .stdin
            .take()
            .context("Failed to open elevated ActionRunner stdin")?;

        let stdout = process
            .stdout
            .take()
            .context("Failed to open elevated ActionRunner stdout")?;

        let stderr = process
            .stderr
            .take()
            .context("Failed to open elevated ActionRunner stderr")?;

        let mut runner = Self {
            process,
            stdin,
            stdout: BufReader::new(stdout),
        };

        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);

            for line in reader.lines() {
                match line {
                    Ok(line) => error!(line),
                    Err(error) => error!(?error, "Failed to read from stderr"),
                }
            }
        });

        loop {
            match runner.read_response()? {
                ProcessResponse::Ready => {
                    debug!("Elevated ActionRunner is ready");
                    break;
                }

                ProcessResponse::NonJson => {}

                response => {
                    bail!("Expected Ready from elevated ActionRunner, got {response:?}");
                }
            }
        }

        Ok(runner)
    }

    pub fn run_action_runner<F>(
        &mut self,
        action_runner: &Arc<ActionRunner>,
        user_context: &UserExecutionContext,
        mut on_progress: F,
    ) -> Result<ActionRunnerResult>
    where
        F: FnMut(Option<String>, Option<i32>, i32, f64, ActionStatus),
    {
        self.send_request(&ProcessRequest::Run {
            action_runner_id: action_runner.get_id(),
            user_context: user_context.clone(),
        })?;

        loop {
            match self.read_response()? {
                ProcessResponse::Ready => {
                    bail!("Unexpected Ready response for elevated ActionRunner");
                }

                ProcessResponse::Started => {
                    debug!(
                        action_runner_id = action_runner.get_id(),
                        "Elevated action started"
                    );
                }

                ProcessResponse::Progress {
                    action,
                    action_nr,
                    total_actions,
                    progress,
                    status,
                } => {
                    on_progress(action, action_nr, total_actions, progress, status);
                }

                ProcessResponse::Finished { results } => {
                    return Ok(results);
                }

                ProcessResponse::Failed { error } => {
                    bail!(error);
                }

                ProcessResponse::NonJson => {}
            }
        }
    }

    fn send_request(&mut self, request: &ProcessRequest) -> Result<()> {
        serde_json::to_writer(&mut self.stdin, request)
            .context("Failed to serialize elevated ActionRunner request")?;

        self.stdin
            .write_all(b"\n")
            .context("Failed to write elevated ActionRunner request")?;

        self.stdin
            .flush()
            .context("Failed to flush elevated ActionRunner request")?;

        Ok(())
    }

    fn read_response(&mut self) -> Result<ProcessResponse> {
        let mut line = String::new();

        let bytes = self
            .stdout
            .read_line(&mut line)
            .context("Failed to read elevated ActionRunner response")?;

        if bytes == 0 {
            // EOF
            let exit_status = self
                .process
                .try_wait()
                .context("Failed to inspect elevated ActionRunner")?;

            bail!("Elevated ActionRunner closed stdout (exit_status: {exit_status:?})");
        }

        line = line.trim().to_string();

        if let Ok(json) = serde_json::from_str(&line) {
            Ok(json)
        } else {
            debug!(line = %line, "Elevated ActionRunner stdout");
            Ok(ProcessResponse::NonJson)
        }
    }

    pub fn _is_running(&mut self) -> Result<bool> {
        Ok(self.process.try_wait()?.is_none())
    }
}

impl Drop for ElevatedActionRunner {
    fn drop(&mut self) {
        debug!("Stopping elevated ActionRunner");

        let _ = serde_json::to_writer(&mut self.stdin, &ProcessRequest::Shutdown);
        let _ = self.stdin.write_all(b"\n");
        let _ = self.stdin.flush();

        if let Ok(None) = self.process.try_wait() {
            let _ = self.process.kill();
        }

        let _ = self.process.wait();
    }
}

fn send_response(response: &ProcessResponse) -> Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    serde_json::to_writer(&mut stdout, response)?;

    stdout.write_all(b"\n")?;
    stdout.flush()?;

    Ok(())
}

fn run_action_runner(action_runner_id: u64, user_context: &UserExecutionContext) -> Result<()> {
    debug!("Finding ActionRunner from id");

    let action_runner = ActionRunner::from_id(action_runner_id, user_context)?;

    send_response(&ProcessResponse::Started)?;

    let result = action_runner.run_actions(Some(&|progress| {
        let response = ProcessResponse::Progress {
            action: progress.action.clone(),
            action_nr: progress.action_nr,
            total_actions: progress.total_actions,
            progress: progress.progress,
            status: progress.status.clone(),
        };

        if let Err(error) = send_response(&response) {
            error!(?error, "Failed to send progress of elevated ActionRunner");
        }
    }));

    match result {
        Ok(results) => {
            send_response(&ProcessResponse::Finished { results })?;
        }

        Err(error) => {
            send_response(&ProcessResponse::Failed {
                error: format!("{error:#}"),
            })?;
        }
    }

    Ok(())
}

pub fn run_elevated_action_runner() -> Result<()> {
    debug!("Starting elevated ActionRunner");

    println!("{}", serde_json::to_string(&ProcessResponse::Ready)?);
    std::io::stdout().flush()?;

    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());

    loop {
        let mut line = String::new();

        let bytes = reader
            .read_line(&mut line)
            .context("Failed to read elevated ActionRunner request")?;

        if bytes == 0 {
            debug!("Parent closed elevated ActionRunner stdin");
            break;
        }

        let request: ProcessRequest = match serde_json::from_str(line.trim()) {
            Ok(request) => request,

            Err(error) => {
                send_response(&ProcessResponse::Failed {
                    error: format!("Invalid elevated ActionRunner request: {error}"),
                })?;

                continue;
            }
        };

        match request {
            ProcessRequest::Shutdown => {
                debug!("Received elevated ActionRunner shutdown");
                break;
            }

            ProcessRequest::Run {
                action_runner_id,
                user_context,
            } => {
                debug!("Received elevated ActionRunner run request");

                if let Err(error) = run_action_runner(action_runner_id, &user_context) {
                    error!(?error, "Elevated ActionRunner failed");

                    send_response(&ProcessResponse::Failed {
                        error: format!("{error:#}"),
                    })?;
                }
            }
        }
    }

    Ok(())
}
