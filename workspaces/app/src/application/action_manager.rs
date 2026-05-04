pub mod action_runner;
pub mod actions;
pub mod elevated_action_runner;

use action_runner::{ActionResult, ActionRunner, ActionStatus};
use anyhow::{Error, Result, anyhow};
use gtk::glib;
use std::{
    fmt::Display,
    sync::{
        Arc,
        mpsc::{self, Sender},
    },
    thread::{self},
};

struct Task {
    name: String,
    runner: ActionRunner,
    on_event: OnEvent,
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
pub trait OnEventFn: Fn(TaskEvent) + Sync + Send + 'static {}
impl<T> OnEventFn for T where T: Fn(TaskEvent) + Sync + Send + 'static {}
type OnEvent = Arc<dyn OnEventFn>;

/// Fifo Action manager
pub struct ActionManager {
    sender: Sender<Task>,
}
impl ActionManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Task>();
        let context = glib::MainContext::default();

        thread::spawn(move || {
            for task in rx {
                // Started
                let on_event = task.on_event.clone();
                let task_name = task.name.clone();

                context.invoke(move || {
                    on_event(TaskEvent::Started { task_name });
                });

                // Run task
                let result = task.runner.run(Some(&|runner_progress| {
                    let callback = task.on_event.clone();
                    let task_name = task.name.clone();
                    let runner_progress = runner_progress.clone();

                    context.invoke(move || {
                        callback(TaskEvent::Progress {
                            task_name,
                            action: runner_progress.action,
                            action_nr: runner_progress.action_nr,
                            total_actions: runner_progress.total_actions,
                            progress: runner_progress.progress,
                            status: runner_progress.status,
                        });
                    });
                }));

                // Result
                let on_event = task.on_event.clone();
                let task_name = task.name.clone();

                let message = match result {
                    Ok(results) => {
                        let mut failed = None;

                        for result in &results {
                            if !result.success {
                                failed = Some(result.stderr.clone());
                                break;
                            }
                        }

                        match failed {
                            None => TaskEvent::Finished { task_name, results },

                            Some(error) => TaskEvent::Failed {
                                task_name,
                                error: anyhow!(error),
                            },
                        }
                    }

                    Err(error) => TaskEvent::Failed { task_name, error },
                };

                context.invoke(move || {
                    on_event(message);
                });
            }
        });

        Self { sender: tx }
    }

    pub fn add<F: OnEventFn>(&self, name: &str, runner: ActionRunner, on_event: F) -> Result<()> {
        match self.sender.send(Task {
            name: name.to_string(),
            runner,
            on_event: Arc::new(on_event),
        }) {
            Ok(()) => Ok(()),
            Err(error) => Err(anyhow!(error.to_string())),
        }
    }
}
