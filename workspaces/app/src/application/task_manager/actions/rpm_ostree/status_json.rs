use anyhow::{Result, bail};
use common::utils;
use serde::{Deserialize, Serialize};
use serde_json;
use std::process::Command;
use tracing::error;

#[derive(Serialize, Deserialize, Debug)]
pub struct Deployment {
    packages: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StatusJson {
    deployments: Vec<Deployment>,
}

impl StatusJson {
    pub fn new() -> Result<Self> {
        let Ok(output) = Command::new("rpm-ostree")
            .arg("status")
            .arg("--json")
            .output()
        else {
            let message = "Failed to run 'rpm-ostree status'";
            error!(message);
            bail!(message);
        };

        if !output.status.success() {
            let message = "Failed to 'rpm-ostree status'";
            let stderr = utils::command::parse_output(&output.stderr);
            error!(stderr, message);
            bail!(message);
        }

        let stdout = utils::command::parse_output(&output.stdout);
        let status_json = match serde_json::from_str(&stdout) {
            Ok(status_json) => status_json,

            Err(error) => {
                let message = "Failed to parse 'rpm-ostree status' as json";
                error!(%error, message);
                bail!(message);
            }
        };

        Ok(status_json)
    }

    pub fn get_packages_latest_deployment(&self) -> Vec<String> {
        self.deployments.first().map_or_else(
            || {
                let message =
                    "Failed to get packages for latest deployments. THIS SHOULD NOT HAPPEN.";
                error!(message);
                Vec::new()
            },
            |a| a.packages.clone(),
        )
    }
}
