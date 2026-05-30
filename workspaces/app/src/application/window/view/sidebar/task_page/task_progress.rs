use crate::application::{
    App,
    task_manager::{TaskEvent, TaskStatus},
};
use gtk::{ProgressBar, prelude::ListBoxRowExt};
use libadwaita::ButtonRow;
use std::rc::Rc;

pub struct TaskProgress {
    task_run_id: Option<String>,
    progress_bar: ProgressBar,
    progress_button: ButtonRow,
}
impl TaskProgress {
    pub fn new(task_run_id: Option<&str>) -> Self {
        let progress_bar = ProgressBar::builder()
            .text(t!("pages.tasks.progress_bar"))
            .show_text(true)
            .fraction(0.0)
            .hexpand(true)
            .build();

        let progress_button = ButtonRow::builder().hexpand(true).build();

        Self {
            task_run_id: task_run_id.map(std::string::ToString::to_string),
            progress_bar,
            progress_button,
        }
    }

    pub fn init(&self, app: &Rc<App>) {
        self.connect_task_manager_progess(app);
    }

    pub fn get_progress_bar(&self) -> &ProgressBar {
        &self.progress_bar
    }

    pub fn get_button_row(&self) -> &ButtonRow {
        self.progress_button.set_child(Some(&self.progress_bar));

        &self.progress_button
    }

    fn connect_task_manager_progess(&self, app: &Rc<App>) {
        let progress_bar_clone = self.progress_bar.clone();
        let task_run_id = self.task_run_id.clone();

        app.task_manager.listen(move |event: &TaskEvent| {
            if let Some(task_run_id) = &task_run_id
                && event.run_id != *task_run_id
            {
                return;
            }

            match &event.status {
                TaskStatus::Progress {
                    action,
                    action_nr,
                    total_actions,
                    progress,
                    status,
                } => {
                    progress_bar_clone.set_fraction(progress.clone());
                }
                _ => {}
            };
        });
    }
}
