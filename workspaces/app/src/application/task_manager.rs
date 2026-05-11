pub mod action_runner;
pub mod actions;
pub mod elevated_action_runner;

use action_runner::{ActionResult, ActionRunner, ActionStatus};
use anyhow::{Error, Result, anyhow};
use async_channel::{Receiver, Sender};
use gtk::glib::{self};
use std::{
    cell::RefCell,
    fmt::Display,
    rc::Rc,
    sync::Arc,
    thread::{self},
};
use tracing::error;

struct Task {
    id: String,
    runner: ActionRunner,
}
impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.runner.name)
    }
}

#[derive(Clone, Debug)]
pub enum TaskStatus {
    Started,
    Progress {
        action: Option<String>,
        action_nr: Option<i32>,
        total_actions: i32,
        progress: f64,
        status: ActionStatus,
    },
    Finished {
        results: Vec<ActionResult>,
    },
    Failed {
        error: Arc<Error>,
    },
}
#[derive(Clone, Debug)]
pub struct TaskEvent {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,
}

struct Listener {
    task_id: Option<String>,
    callback: Rc<dyn Fn(&TaskEvent)>,
}

/// Fifo Action manager
pub struct TaskManager {
    task_sender: Sender<Task>,
    event_receiver: Receiver<TaskEvent>,
    listeners: Rc<RefCell<Vec<Listener>>>,
}
impl TaskManager {
    pub fn new() -> Rc<Self> {
        let (task_sender, task_receiver) = async_channel::unbounded();
        let (event_sender, event_receiver) = async_channel::unbounded();

        Self::run_actions_thread(task_receiver, event_sender);

        Rc::new(Self {
            task_sender,
            event_receiver,
            listeners: Rc::new(RefCell::new(Vec::new())),
        })
    }

    pub fn init(self: &Rc<Self>) {
        self.connect_listeners();
    }

    fn run_actions_thread(task_receiver: Receiver<Task>, event_sender: Sender<TaskEvent>) {
        thread::spawn(move || {
            while let Ok(task) = task_receiver.recv_blocking() {
                // Started
                let task_name = task.runner.name.clone();
                let task_id = task.id.clone();

                let _ = event_sender.send_blocking(TaskEvent {
                    id: task_id,
                    name: task_name,
                    status: TaskStatus::Started,
                });

                // Run task
                let task_name = task.runner.name.clone();

                let result = task.runner.run(Some(&|runner_progress| {
                    let task_name = task_name.clone();
                    let task_id = task.id.clone();
                    let runner_progress = runner_progress.clone();

                    let _ = event_sender.send_blocking(TaskEvent {
                        id: task_id,
                        name: task_name,
                        status: TaskStatus::Progress {
                            action: runner_progress.action,
                            action_nr: runner_progress.action_nr,
                            total_actions: runner_progress.total_actions,
                            progress: runner_progress.progress,
                            status: runner_progress.status,
                        },
                    });
                }));

                // Result
                let task_name = task_name.clone();
                let task_id = task.id.clone();

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
                            None => TaskEvent {
                                id: task_id,
                                name: task_name,
                                status: TaskStatus::Finished { results },
                            },

                            Some(error) => TaskEvent {
                                id: task_id,
                                name: task_name,
                                status: TaskStatus::Failed {
                                    error: Arc::new(anyhow!(error)),
                                },
                            },
                        }
                    }

                    Err(error) => TaskEvent {
                        id: task_id,
                        name: task_name,
                        status: TaskStatus::Failed {
                            error: Arc::new(error),
                        },
                    },
                };

                let _ = event_sender.send_blocking(message);
            }
        });
    }

    fn connect_listeners(self: &Rc<Self>) {
        let self_clone = self.clone();

        glib::spawn_future_local(async move {
            while let Ok(event) = self_clone.event_receiver.recv().await {
                let mut done_task_indices = Vec::new();
                let mut listeners_borrow = self_clone.listeners.borrow_mut();

                for (i, listener) in listeners_borrow.iter().enumerate() {
                    if let Some(task_id) = &listener.task_id {
                        if event.id != *task_id {
                            continue;
                        }
                        match event.status {
                            TaskStatus::Finished { results: _ }
                            | TaskStatus::Failed { error: _ } => {
                                done_task_indices.push(i);
                            }
                            _ => {}
                        }
                    }

                    (listener.callback)(&event);
                }
                for index in done_task_indices {
                    listeners_borrow.remove(index);
                }
            }
        });
    }

    pub fn add<F: Fn(&TaskEvent) + 'static>(
        self: &Rc<Self>,
        runner: ActionRunner,
        on_event: F,
    ) -> Result<()> {
        let name = &runner.name.clone();
        let task = Task {
            id: runner.name.clone(),
            runner,
        };

        self.listeners.borrow_mut().push(Listener {
            task_id: Some(task.id.clone()),
            callback: Rc::new(on_event),
        });

        match self.task_sender.send_blocking(task) {
            Ok(()) => Ok(()),
            Err(error) => {
                error!(name, %error, "Failed to run task");
                Err(anyhow!(error.to_string()))
            }
        }
    }

    pub fn listen<F: Fn(&TaskEvent) + 'static>(self: &Rc<Self>, on_event: F) {
        self.listeners.borrow_mut().push(Listener {
            task_id: None,
            callback: Rc::new(on_event),
        });
    }
}
