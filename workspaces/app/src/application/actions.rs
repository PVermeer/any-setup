pub mod systemd;

use crate::application::actions::systemd::SystemdAction;
use anyhow::{Context, Result};
use common::utils;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::{Command, Output};
use tracing::debug;

pub trait IsAction {
    fn get_command(&self) -> Command;
    fn needs_elevation(&self) -> bool;
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Action {
    SystemD(SystemdAction),
}
impl IsAction for Action {
    fn get_command(&self) -> Command {
        match self {
            Self::SystemD(action) => action.get_command(),
        }
    }

    fn needs_elevation(&self) -> bool {
        match self {
            Self::SystemD(action) => action.needs_elevation(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActionResult {
    action: Action,
    success: bool,
    stdout: String,
    stderr: String,
}
impl ActionResult {
    fn from_output(action: Action, output: &Output) -> Self {
        Self {
            action,
            success: output.status.success(),
            stdout: utils::command::parse_output(&output.stdout),
            stderr: utils::command::parse_output(&output.stderr),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActionRunner {
    queue: Vec<Action>,
    elevate: bool,
}
impl ActionRunner {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            elevate: false,
        }
    }

    pub fn add(&mut self, action: Action) {
        if action.needs_elevation() {
            self.elevate = true;
        }
        self.queue.push(action);
    }

    pub fn run(self) -> Result<Vec<ActionResult>> {
        let action_results = if self.elevate {
            self.run_elevated()
        } else {
            self.run_actions()
        };

        debug!(?action_results, "Action results");

        action_results
    }

    pub fn run_actions(self) -> Result<Vec<ActionResult>> {
        let mut results = Vec::new();

        for action in self.queue {
            let mut command = action.get_command();
            let output = command.output().context("Failed to run command")?;
            let action_result = ActionResult::from_output(action, &output);
            results.push(action_result);
        }

        Ok(results)
    }

    fn run_elevated(&self) -> Result<Vec<ActionResult>> {
        let json = json!(&self).to_string();

        let current_exe = std::env::current_exe()?;
        let mut command = "pkexec";
        if utils::env::is_devcontainer() {
            command = "sudo";
        }

        let output = Command::new(command)
            .arg(current_exe)
            .arg("run-batch")
            .arg("--json")
            .arg(json)
            .output()
            .context("Failed to run command")?;
        let action_results: Vec<ActionResult> = serde_json::from_slice(&output.stdout)?;

        Ok(action_results)
    }
}
