use crate::application::{
    App,
    task_manager::{TaskEvent, TaskStatus},
};
use gtk::{ProgressBar, prelude::WidgetExt};
use libadwaita::ButtonRow;
use std::rc::Rc;

pub struct TaskProgress {
    pub progress_bar: ProgressBar,
    pub progress_button: ButtonRow,
}
impl TaskProgress {
    pub fn new() -> Self {
        let progress_bar = ProgressBar::builder()
            .text(t!("sidebar.progress_bar"))
            .show_text(true)
            .fraction(0.0)
            .build();
        progress_bar.set_hexpand(true);

        let progress_button = ButtonRow::builder()
            .child(&progress_bar)
            .hexpand(true)
            .build();

        Self {
            progress_bar,
            progress_button,
        }
    }

    pub fn init(&self, app: &Rc<App>) {
        self.connect_task_manager_progess(app);
    }

    fn connect_task_manager_progess(&self, app: &Rc<App>) {
        let progress_bar_clone = self.progress_bar.clone();

        app.task_manager.listen(move |event: &TaskEvent| {
            dbg!("From progress bar!", event);

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
