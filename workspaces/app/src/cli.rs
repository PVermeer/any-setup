use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf};

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<AppCommand>,

    /// Set custom config pages directory
    #[arg(short, long)]
    pub pages_dir: Option<PathBuf>,
}
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum AppCommand {
    /// Run batched commands
    ActionRunner,
}
// Why does clap not implement this?
impl Display for AppCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActionRunner => write!(f, "action-runner"),
        }
    }
}
