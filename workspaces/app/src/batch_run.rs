use crate::application::actions::ActionRunner;
use anyhow::Result;
use clap::Parser;
use serde_json::json;

#[derive(Parser, Debug)]
pub struct BatchArgs {
    /// Json input
    #[arg(long)]
    pub json: String,
}

pub fn run_batch_actions(args: &BatchArgs) -> Result<()> {
    let action_runner: ActionRunner = serde_json::from_str(&args.json)?;
    let output = action_runner.run_actions()?;
    let json = json!(output).to_string();
    println!("{json}");

    Ok(())
}
