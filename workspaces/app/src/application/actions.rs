pub mod systemd;

use crate::application::actions::systemd::SystemdAction;
use anyhow::Result;
use common::utils;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::VecDeque, process::Command};

pub trait IsAction {
    fn into_command(self) -> Command;
    fn needs_elevation(&self) -> bool;
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Action {
    SystemD(SystemdAction),
}
impl IsAction for Action {
    fn into_command(self) -> Command {
        match self {
            Self::SystemD(action) => action.into_command(),
        }
    }

    fn needs_elevation(&self) -> bool {
        match self {
            Self::SystemD(action) => action.needs_elevation(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActionRunner {
    queue: VecDeque<Action>,
    elevate: bool,
}
impl ActionRunner {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            elevate: false,
        }
    }

    pub fn add(&mut self, action: Action) {
        if action.needs_elevation() {
            self.elevate = true;
        }
        self.queue.push_back(action);
    }

    pub fn run(&mut self) -> Result<()> {
        if self.elevate {
            self.run_elevated()?;
        } else {
            self.run_actions()?;
        }

        Ok(())
    }

    pub fn run_actions(&mut self) -> Result<()> {
        while let Some(action) = self.queue.pop_front() {
            let mut cmd = action.into_command();
            cmd.spawn()?;
        }

        Ok(())
    }

    fn run_elevated(&self) -> Result<()> {
        let json = json!(&self).to_string();

        let current_exe = std::env::current_exe()?;
        let mut command = "pkexec";
        if utils::env::is_devcontainer() {
            command = "sudo";
        }

        Command::new(command)
            .arg(current_exe)
            .arg("run-batch")
            .arg("--json")
            .arg(json)
            .spawn()?;

        Ok(())
    }
}
