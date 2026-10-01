use crate::application::{
    App,
    task_manager::{TaskEvent, TaskStatus},
};
use gtk::{
    Image, Orientation, ProgressBar,
    prelude::{BoxExt, ListBoxRowExt, WidgetExt},
};
use libadwaita::ActionRow;
use std::rc::Rc;

pub struct TaskProgress {
    task_run_id: Option<String>,
    for_all_tasks: bool,
    progress_bar: ProgressBar,
    progress_row: ActionRow,
    progress_row_content: gtk::Box,
    progress_row_warning: Image,
}
impl TaskProgress {
    pub fn new_as_row(task_run_id: Option<&str>) -> Self {
        let this = Self::build_self(task_run_id);

        this.progress_row
            .set_child(Some(&this.progress_row_content));
        this.progress_row_content.append(&this.progress_row_warning);
        this.progress_row_content.append(&this.progress_bar);

        this
    }

    pub fn new_as_progress_bar(task_run_id: Option<&str>) -> Self {
        Self::build_self(task_run_id)
    }

    pub fn init(&self, app: &Rc<App>) {
        self.connect_task_manager_progess(app);
    }

    fn build_self(task_run_id: Option<&str>) -> Self {
        let progress_bar = ProgressBar::builder()
            .text(t!("pages.tasks.progress_bar"))
            .show_text(true)
            .fraction(0.0)
            .margin_top(20)
            .margin_bottom(20)
            .hexpand(true)
            .build();

        let progress_row = ActionRow::builder().activatable(true).build();
        let progress_row_content = gtk::Box::new(Orientation::Horizontal, 10);
        let progress_row_warning = Image::builder()
            .icon_name("dialog-warning-symbolic")
            .css_classes(["warning"])
            .margin_top(12)
            .visible(false)
            .build();

        let task_run_id = task_run_id.map(std::string::ToString::to_string);
        let for_all_tasks = task_run_id.is_none();

        Self {
            task_run_id,
            for_all_tasks,
            progress_bar,
            progress_row,
            progress_row_content,
            progress_row_warning,
        }
    }

    pub fn get_progress_bar(&self) -> ProgressBar {
        self.progress_bar.clone()
    }

    pub fn get_progress_row(&self) -> ActionRow {
        self.progress_row.clone()
    }

    fn connect_task_manager_progess(&self, app: &Rc<App>) {
        let for_all_tasks = self.for_all_tasks;
        let progress_row_warning_clone = self.progress_row_warning.clone();
        let progress_bar_clone = self.progress_bar.clone();
        let task_run_id = self.task_run_id.clone();
        let progress_bar_text = t!("pages.tasks.progress_bar");

        app.task_manager
            .listen(task_run_id, move |event: &TaskEvent| {
                progress_bar_clone.set_text(Some(&format!(
                    "{progress_bar_text} · {} {}",
                    event.tasks_in_queue + 1,
                    t!("pages.tasks.in_queue")
                )));

                match &event.status {
                    TaskStatus::Added => {
                        if for_all_tasks {
                            progress_bar_clone.remove_css_class("error");
                        }
                    }

                    TaskStatus::Started => {}

                    TaskStatus::Progress { progress, .. } => {
                        progress_bar_clone.set_fraction(*progress);
                    }

                    TaskStatus::Finished { results: _ } | TaskStatus::Failed { error: _ } => {
                        progress_bar_clone.set_text(Some(&t!("pages.tasks.progress_bar")));
                        progress_bar_clone.set_fraction(1.0);

                        let success = match &event.status {
                            TaskStatus::Finished { results } => results.success,
                            _ => false,
                        };

                        if for_all_tasks && !success {
                            progress_bar_clone.add_css_class("error");
                            progress_row_warning_clone.set_visible(true);
                        }
                    }
                }
            });
    }
}
