use std::fmt::Display;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<AppCommand>,
}
#[derive(Subcommand)]
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
