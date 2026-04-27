use crate::application::actions::ActionRunner;
use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct BatchArgs {
    /// Json input
    #[arg(long)]
    pub json: String,
}

pub fn run_batch_actions(args: &BatchArgs) -> Result<()> {
    let mut action_runner: ActionRunner = serde_json::from_str(&args.json)?;
    action_runner.run_actions()
}
