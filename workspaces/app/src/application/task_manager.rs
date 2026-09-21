pub mod action_runner;
pub mod actions;
pub mod elevated_action_runner;
pub mod user_execution_context;

use action_runner::{ActionResult, ActionRunner, ActionStatus};
use anyhow::{Error, Result, anyhow, bail};
use async_channel::{Receiver, Sender};
use common::utils;
use elevated_action_runner::ElevatedActionRunner;
use gtk::glib;
use rand::{
    distr::{Alphanumeric, SampleString},
    rng,
};
use std::{
    cell::RefCell, collections::HashSet, fmt::Display, rc::Rc, sync::Arc, thread, time::Duration,
};
use tracing::{debug, error, warn};
use user_execution_context::UserExecutionContext;

#[derive(Debug)]
struct Task {
    id: u64,
    run_id: String,
    runner: Arc<ActionRunner>,
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.runner.name)
    }
}

enum ElevatedActionRunnerCommand {
    EnsureStarted,
    Run(Task),
}

#[derive(Clone, Debug)]
pub enum TaskStatus {
    Added,
    Started,
    Progress {
        action: Option<String>,
        action_nr: Option<i32>,
        total_actions: i32,
        progress: f64,
        _status: ActionStatus,
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
    pub id: u64,
    pub run_id: String,
    pub name: String,
    pub status: TaskStatus,
    pub tasks_in_queue: u32,
    task_receiver: Receiver<Task>,
}

impl TaskEvent {
    fn new(
        id: u64,
        run_id: String,
        name: String,
        status: TaskStatus,
        task_receiver: &Receiver<Task>,
    ) -> Self {
        Self {
            id,
            run_id,
            name,
            status,
            tasks_in_queue: u32::try_from(task_receiver.len()).unwrap_or_default(),
            task_receiver: task_receiver.clone(),
        }
    }

    pub fn with_status(&self, status: TaskStatus) -> Self {
        let mut self_clone = self.clone();
        self_clone.status = status;
        self_clone.tasks_in_queue =
            u32::try_from(self_clone.task_receiver.len()).unwrap_or_default();
        self_clone
    }
}

struct Listener {
    run_id: Option<String>,
    callback: Rc<dyn Fn(&TaskEvent)>,
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let location = std::panic::Location::caller();

        f.debug_struct("Listener")
            .field("run_id", &self.run_id)
            .field(
                "callback",
                &format!("{}:{}", location.file(), location.line()),
            )
            .finish()
    }
}

pub struct TaskManager {
    task_sender: Sender<Task>,
    task_receiver: Receiver<Task>,
    elevated_sender: Sender<ElevatedActionRunnerCommand>,
    event_sender: Sender<TaskEvent>,
    event_receiver: Receiver<TaskEvent>,
    active_tasks: Rc<RefCell<HashSet<u64>>>,
    listeners: Rc<RefCell<Vec<Listener>>>,
}
impl TaskManager {
    pub fn new(user_context: &UserExecutionContext) -> Rc<Self> {
        let (task_sender, task_receiver) = async_channel::unbounded();
        let (elevated_sender, elevated_receiver) = async_channel::unbounded();
        let (elevated_result_sender, elevated_result_receiver) = async_channel::unbounded();
        let (event_sender, event_receiver) = async_channel::unbounded();

        Self::run_actions_thread(
            task_receiver.clone(),
            elevated_sender.clone(),
            elevated_result_receiver.clone(),
            event_sender.clone(),
        );

        Self::run_elevated_thread(
            elevated_receiver,
            elevated_result_sender,
            event_sender.clone(),
            user_context,
            &task_receiver,
        );

        Rc::new(Self {
            task_sender,
            task_receiver,
            elevated_sender,
            event_sender,
            event_receiver,
            active_tasks: Rc::new(RefCell::new(HashSet::new())),
            listeners: Rc::new(RefCell::new(Vec::new())),
        })
    }

    pub fn init(self: &Rc<Self>) {
        self.connect_listeners();
        self.connect_active_tasks();
    }

    pub fn add<F>(self: &Rc<Self>, runner: &Arc<ActionRunner>, on_event: F) -> Result<String>
    where
        F: Fn(&TaskEvent) + 'static,
    {
        let name = runner.name.clone();
        let id = runner.get_id();

        let run_id = format!("{id}-{}", Alphanumeric.sample_string(&mut rng(), 8));

        let task = Task {
            id,
            run_id: run_id.clone(),
            runner: runner.clone(),
        };

        debug!(?task, "Adding task");

        if !self.active_tasks.borrow_mut().insert(id) {
            let message = "Task already in queue";

            warn!(task = name, "{message}");

            bail!(message);
        }

        self.listeners.borrow_mut().push(Listener {
            run_id: Some(run_id.clone()),
            callback: Rc::new(on_event),
        });

        if runner.needs_elevation() {
            self.elevated_sender
                .send_blocking(ElevatedActionRunnerCommand::EnsureStarted)
                .map_err(|error| {
                    let _ = self.active_tasks.borrow_mut().remove(&id);
                    error!(?error, "Failed to request elevated runner startup");
                    anyhow!("Failed to request elevated runner startup: {error}")
                })?;
        }

        self.task_sender.send_blocking(task).map_err(|error| {
            let _ = self.active_tasks.borrow_mut().remove(&id);
            error!(?error, "Failed to add task");
            anyhow!("Failed to add task: {error}")
        })?;

        self.event_sender
            .send_blocking(TaskEvent::new(
                id,
                run_id.clone(),
                name,
                TaskStatus::Added,
                &self.task_receiver,
            ))
            .map_err(|error| {
                error!(?error, "Failed to send add task event");
                anyhow!("Failed to send add task event: {error}")
            })?;

        Ok(run_id)
    }

    pub fn listen<F>(self: &Rc<Self>, run_id: Option<String>, on_event: F)
    where
        F: Fn(&TaskEvent) + 'static,
    {
        let listener = Listener {
            run_id,
            callback: Rc::new(on_event),
        };

        debug!(?listener, "Adding task listener");

        self.listeners.borrow_mut().push(listener);
    }

    pub fn is_running(&self) -> bool {
        !self.active_tasks.borrow().is_empty()
    }

    fn run_actions_thread(
        task_receiver: Receiver<Task>,
        elevated_sender: Sender<ElevatedActionRunnerCommand>,
        elevated_result_receiver: Receiver<(String, Result<Vec<ActionResult>>)>,
        event_sender: Sender<TaskEvent>,
    ) {
        thread::spawn(move || {
            while let Ok(task) = task_receiver.recv_blocking() {
                let task_event = TaskEvent::new(
                    task.id,
                    task.run_id.clone(),
                    task.runner.name.clone(),
                    TaskStatus::Started,
                    &task_receiver,
                );

                debug!(
                    task = %task,
                    elevated = task.runner.needs_elevation(),
                    "Starting task"
                );

                let result = if task.runner.needs_elevation() {
                    let run_id = task.run_id.clone();

                    let _ = event_sender.send_blocking(task_event.with_status(TaskStatus::Started));

                    if let Err(error) =
                        elevated_sender.send_blocking(ElevatedActionRunnerCommand::Run(task))
                    {
                        error!(
                            ?error,
                            run_id = %run_id,
                            "Failed to send task to elevated ActionRunner"
                        );

                        Err(anyhow!(
                            "Failed to send task to elevated ActionRunner: {error}"
                        ))
                    } else {
                        loop {
                            let Ok((result_run_id, result)) =
                                elevated_result_receiver.recv_blocking()
                            else {
                                error!(
                                    run_id = %run_id,
                                    "Elevated runner stopped before returning a result"
                                );

                                break Err(anyhow!("Elevated runner stopped unexpectedly"));
                            };

                            if result_run_id == run_id {
                                break result;
                            }
                        }
                    }
                } else {
                    let _ = event_sender.send_blocking(task_event.with_status(TaskStatus::Started));

                    task.runner.run_actions(Some(&|progress| {
                        let event = task_event.with_status(TaskStatus::Progress {
                            action: progress.action.clone(),
                            action_nr: progress.action_nr,
                            total_actions: progress.total_actions,
                            progress: progress.progress,
                            _status: progress.status.clone(),
                        });

                        let _ = event_sender.send_blocking(event);
                    }))
                };

                if cfg!(debug_assertions) && utils::env::is_devcontainer() {
                    std::thread::sleep(Duration::from_secs(10));
                }

                let event = match result {
                    Ok(results) => task_event.with_status(TaskStatus::Finished { results }),

                    Err(error) => task_event.with_status(TaskStatus::Failed {
                        error: Arc::new(error),
                    }),
                };

                let _ = event_sender.send_blocking(event);
            }

            debug!("Task worker stopped");
        });
    }

    fn run_elevated_thread(
        elevated_receiver: Receiver<ElevatedActionRunnerCommand>,
        elevated_result_sender: Sender<(String, Result<Vec<ActionResult>>)>,
        event_sender: Sender<TaskEvent>,
        user_context: &UserExecutionContext,
        task_receiver: &Receiver<Task>,
    ) {
        let user_context = user_context.clone();
        let task_receiver = task_receiver.clone();

        thread::spawn(move || {
            let mut elevated_runner: Option<ElevatedActionRunner> = None;

            while let Ok(command) = elevated_receiver.recv_blocking() {
                match command {
                    ElevatedActionRunnerCommand::EnsureStarted => {
                        if elevated_runner.is_some() {
                            continue;
                        }

                        debug!("Starting elevated action runner");

                        match ElevatedActionRunner::start(&user_context) {
                            Ok(runner) => {
                                debug!("Elevated action runner started");

                                elevated_runner = Some(runner);
                            }

                            Err(error) => {
                                error!(?error, "Failed to start elevated runner");
                            }
                        }
                    }

                    ElevatedActionRunnerCommand::Run(task) => {
                        let run_id = task.run_id.clone();

                        if elevated_runner.is_none() {
                            debug!(
                                run_id = %run_id,
                                "Elevated runner was not started; starting now"
                            );

                            match ElevatedActionRunner::start(&user_context) {
                                Ok(runner) => {
                                    elevated_runner = Some(runner);
                                }

                                Err(error) => {
                                    error!(
                                        ?error,
                                        run_id = %run_id,
                                        "Failed to start elevated runner"
                                    );

                                    let _ =
                                        elevated_result_sender.send_blocking((run_id, Err(error)));

                                    continue;
                                }
                            }
                        }

                        let Some(elevated_runner) = elevated_runner.as_mut() else {
                            let _ = elevated_result_sender.send_blocking((
                                run_id,
                                Err(anyhow!("Elevated runner is unavailable")),
                            ));

                            continue;
                        };

                        let task_event = TaskEvent::new(
                            task.id,
                            task.run_id.clone(),
                            task.runner.name.clone(),
                            TaskStatus::Started,
                            &task_receiver,
                        );

                        let event_sender_clone = event_sender.clone();
                        let task_event_clone = task_event.clone();

                        let result = elevated_runner.run_action_runner(
                            &task.runner,
                            &user_context,
                            |action, action_nr, total_actions, progress, status| {
                                let event =
                                    task_event_clone.clone().with_status(TaskStatus::Progress {
                                        action,
                                        action_nr,
                                        total_actions,
                                        progress,
                                        _status: status,
                                    });

                                let _ = event_sender_clone.send_blocking(event);
                            },
                        );

                        let _ = elevated_result_sender.send_blocking((run_id, result));
                    }
                }
            }

            drop(elevated_runner);

            debug!("Elevated runner stopped");
        });
    }

    fn connect_listeners(self: &Rc<Self>) {
        let self_clone = self.clone();

        glib::spawn_future_local(async move {
            while let Ok(event) = self_clone.event_receiver.recv().await {
                let (callbacks_to_run, done_task_run_ids) = {
                    let listeners = self_clone.listeners.borrow();

                    let mut callbacks_to_run = Vec::new();
                    let mut done_task_run_ids = HashSet::new();

                    for listener in listeners.iter() {
                        if let Some(run_id) = &listener.run_id {
                            if event.run_id != *run_id {
                                continue;
                            }

                            match event.status {
                                TaskStatus::Finished { .. } | TaskStatus::Failed { .. } => {
                                    done_task_run_ids.insert(event.run_id.clone());
                                }

                                _ => {}
                            }
                        }

                        callbacks_to_run.push(listener.callback.clone());
                    }

                    (callbacks_to_run, done_task_run_ids)
                };

                for callback in callbacks_to_run {
                    callback(&event);
                }

                self_clone.listeners.borrow_mut().retain(|listener| {
                    !listener
                        .run_id
                        .as_ref()
                        .is_some_and(|run_id| done_task_run_ids.contains(run_id))
                });
            }
        });
    }

    fn connect_active_tasks(self: &Rc<Self>) {
        let self_clone = self.clone();

        self.listen(None, move |event| {
            if matches!(
                event.status,
                TaskStatus::Failed { .. } | TaskStatus::Finished { .. }
            ) {
                let _ = self_clone.active_tasks.borrow_mut().remove(&event.id);
            }
        });
    }
}
