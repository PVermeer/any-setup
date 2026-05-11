use crate::application::task_manager::action_runner::{ActionJsonMessage, ActionRunner};
use anyhow::Result;
use clap::Parser;
use serde_json::json;

#[derive(Parser, Debug)]
pub struct BatchArgs {
    /// Json input
    #[arg(long)]
    pub json: String,
}

pub fn elevated_action_runner(args: &BatchArgs) -> Result<()> {
    let action_runner: ActionRunner = serde_json::from_str(&args.json)?;
    let results = action_runner.run(None)?;
    let message = ActionJsonMessage::ActionResults(results);
    let json_results = json!(message).to_string();
    println!("{json_results}");

    Ok(())
}
