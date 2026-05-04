pub mod action_runner;
pub mod actions;
pub mod elevated_action_runner;

use crate::application::action_manager::action_runner::{ActionResult, ActionRunner, ActionStatus};
use anyhow::{Error, Result, anyhow};
use crossbeam_channel::{Sender, unbounded};
use gtk::glib;
use std::{
    fmt::Display,
    sync::Arc,
    thread::{self},
};

struct Task {
    name: String,
    runner: ActionRunner,
    callback: Callback,
}
impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug)]
pub enum TaskEvent {
    Started {
        task_name: String,
    },
    Progress {
        task_name: String,
        action: Option<String>,
        action_nr: Option<i32>,
        total_actions: i32,
        progress: f64,
        status: ActionStatus,
    },
    Finished {
        task_name: String,
        results: Vec<ActionResult>,
    },
    Failed {
        task_name: String,
        error: Error,
    },
}
pub trait CallbackFn: Fn(TaskEvent) + Sync + Send + 'static {}
impl<T> CallbackFn for T where T: Fn(TaskEvent) + Sync + Send + 'static {}
type Callback = Arc<dyn CallbackFn>;

/// Fifo Action manager
pub struct ActionManager {
    sender: Sender<Task>,
}
impl ActionManager {
    pub fn new() -> Self {
        let (tx, rx) = unbounded::<Task>();
        let context = glib::MainContext::default();

        thread::spawn(move || {
            for task in rx {
                // Started
                let callback = task.callback.clone();
                let task_name = task.name.clone();

                context.invoke(move || {
                    callback(TaskEvent::Started { task_name });
                });

                // Run task
                let result = task.runner.run(Some(&|runner_progress| {
                    let callback = task.callback.clone();
                    let task_name = task.name.clone();
                    let action_progress = runner_progress.clone();

                    context.invoke(move || {
                        callback(TaskEvent::Progress {
                            task_name,
                            action: action_progress.action,
                            action_nr: action_progress.action_nr,
                            total_actions: action_progress.total_actions,
                            progress: action_progress.progress,
                            status: action_progress.status,
                        });
                    });
                }));

                // Result
                let callback = task.callback.clone();
                let action = task.name.clone();

                match result {
                    Ok(results) => {
                        let mut has_failed = false;
                        for result in &results {
                            if !result.success {
                                callback(TaskEvent::Failed {
                                    task_name: action.clone(),
                                    error: anyhow!(result.stderr.clone()),
                                });
                                has_failed = true;
                                break;
                            }
                        }
                        if !has_failed {
                            callback(TaskEvent::Finished {
                                task_name: action,
                                results,
                            });
                        }
                    }
                    Err(error) => {
                        callback(TaskEvent::Failed {
                            task_name: action,
                            error,
                        });
                    }
                }
            }
        });

        Self { sender: tx }
    }

    pub fn add<F: CallbackFn>(
        &self,
        name: &str,
        runner: ActionRunner,
        on_action_event: F,
    ) -> Result<()> {
        match self.sender.send(Task {
            name: name.to_string(),
            runner,
            callback: Arc::new(on_action_event),
        }) {
            Ok(()) => Ok(()),
            Err(error) => Err(anyhow!(error.to_string())),
        }
    }
}
