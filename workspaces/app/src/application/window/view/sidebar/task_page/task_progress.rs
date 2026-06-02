use crate::application::{
    App,
    task_manager::{TaskEvent, TaskStatus},
};
use gtk::{ProgressBar, prelude::ListBoxRowExt};
use libadwaita::ActionRow;
use std::rc::Rc;

pub struct TaskProgress {
    task_run_id: Option<String>,
    progress_bar: ProgressBar,
    progress_row: ActionRow,
}
impl TaskProgress {
    pub fn new(task_run_id: Option<&str>) -> Self {
        let progress_bar = ProgressBar::builder()
            .text(t!("pages.tasks.progress_bar"))
            .show_text(true)
            .fraction(0.0)
            .build();

        let progress_row = ActionRow::builder().activatable(true).build();

        Self {
            task_run_id: task_run_id.map(std::string::ToString::to_string),
            progress_bar,
            progress_row,
        }
    }

    pub fn init(&self, app: &Rc<App>) {
        self.connect_task_manager_progess(app);
    }

    pub fn get_progress_bar(&self) -> &ProgressBar {
        &self.progress_bar
    }

    pub fn get_progress_row(&self) -> &ActionRow {
        self.progress_row.set_child(Some(&self.progress_bar));

        &self.progress_row
    }

    fn connect_task_manager_progess(&self, app: &Rc<App>) {
        let progress_bar_clone = self.progress_bar.clone();
        let task_run_id = self.task_run_id.clone();
        let progress_bar_text = t!("pages.tasks.progress_bar");

        app.task_manager
            .listen(task_run_id, move |event: &TaskEvent| {
                progress_bar_clone.set_text(Some(&format!(
                    "{progress_bar_text} ({})",
                    event.tasks_in_queue + 1
                )));

                match &event.status {
                    TaskStatus::Started => {}
                    TaskStatus::Progress {
                        action,
                        action_nr,
                        total_actions,
                        progress,
                        status,
                    } => {
                        progress_bar_clone.set_fraction(progress.clone());
                    }
                    TaskStatus::Failed { error: _ } | TaskStatus::Finished { results: _ } => {
                        progress_bar_clone.set_text(Some(&format!("{progress_bar_text} ({})", 0)));
                    }
                }
            });
    }
}
