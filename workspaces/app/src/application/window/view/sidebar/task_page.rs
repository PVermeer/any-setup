pub mod task_progress;
mod task_vec;
mod task_view_page;

use super::NavPage;
use crate::application::{
    App,
    pages::PrefNavPageBuild,
    task_manager::{TaskEvent, TaskStatus, action_runner::ActionResult},
};
use gtk::{Image, prelude::WidgetExt};
use libadwaita::{
    ActionRow, NavigationPage, NavigationView, PreferencesGroup, Spinner,
    prelude::{ActionRowExt, PreferencesGroupExt, PreferencesPageExt},
};
use std::{cell::RefCell, rc::Rc};
use task_vec::{TaskVec, TaskVecExt};
use task_view_page::TaskViewPage;
use tracing::error;

#[derive(Debug)]
pub struct TaskUi {
    task_row: ActionRow,
    run_id: String,
    success_icon: Image,
    fail_icon: Image,
    running_icon: Spinner,
    results: Option<Vec<ActionResult>>,
}
impl TaskUi {
    fn from_event(task_event: &TaskEvent, nav_view: &NavigationView, app: &Rc<App>) -> Self {
        let id = task_event.run_id.clone();
        let (task_row, success_icon, fail_icon, running_icon) =
            Self::build_task_row(app, task_event, nav_view);

        Self {
            task_row,
            run_id: id,
            success_icon,
            fail_icon,
            running_icon,
            results: None,
        }
    }

    fn set_progress(&mut self, action: Option<&str>, action_nr: Option<i32>, total_actions: i32) {
        let Some(action) = action else {
            self.task_row.set_subtitle("");
            return;
        };

        let progress = if let Some(task_nr) = action_nr {
            format!("{task_nr} / {total_actions} : {action}")
        } else {
            String::new()
        };

        self.task_row.set_subtitle(&progress);
    }

    fn set_success(&mut self, results: &[ActionResult]) {
        self.results = Some(results.to_owned());

        self.task_row.set_subtitle("");
        self.success_icon.set_visible(true);
        self.fail_icon.set_visible(false);
        self.running_icon.set_visible(false);
    }

    fn set_error(&mut self, error: &anyhow::Error) {
        self.task_row.set_subtitle(&error.to_string());
        self.task_row.add_css_class("error");

        self.success_icon.set_visible(false);
        self.fail_icon.set_visible(true);
        self.running_icon.set_visible(false);
    }

    fn build_task_row(
        app: &Rc<App>,
        task_event: &TaskEvent,
        nav_view: &NavigationView,
    ) -> (ActionRow, Image, Image, Spinner) {
        let title = &task_event.name;
        let task_row = ActionRow::builder().title(title).activatable(true).build();

        let success_icon = Image::builder()
            .icon_name("object-select-symbolic")
            .css_classes(["success"])
            .visible(false)
            .build();
        let fail_icon = Image::builder()
            .icon_name("process-stop-symbolic")
            .css_classes(["error"])
            .visible(false)
            .build();
        let running_icon = Spinner::new();

        task_row.add_prefix(&Image::from_icon_name("system-run-symbolic"));
        task_row.add_suffix(&running_icon);
        task_row.add_suffix(&success_icon);
        task_row.add_suffix(&fail_icon);

        let event_clone = task_event.clone();
        let nav_view_clone = nav_view.clone();

        let details_page = TaskViewPage::from_event(&event_clone).init(app);

        task_row.connect_activated(move |_task_row| {
            nav_view_clone.push(details_page.get_navpage());
        });

        (task_row, success_icon, fail_icon, running_icon)
    }
}

pub struct TaskPage {
    nav_page: NavigationPage,
    nav_view: NavigationView,
    tasks_pref_group: PreferencesGroup,
    tasks: RefCell<TaskVec>,
}
impl NavPage for TaskPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_section(&self) -> Option<&str> {
        None
    }

    fn get_icon(&self) -> Option<&str> {
        None
    }
}
impl TaskPage {
    pub fn new() -> Rc<Self> {
        let PrefNavPageBuild {
            nav_page,
            nav_view,
            prefs_page,
        } = Self::build_preferences_nav_page(&t!("pages.tasks.title"));

        let tasks_pref_group = PreferencesGroup::builder()
            .title(t!("pages.tasks.title"))
            .build();

        prefs_page.add(&tasks_pref_group);

        Rc::new(Self {
            nav_page,
            nav_view,
            tasks_pref_group,
            tasks: RefCell::new(TaskVec(Vec::new())),
        })
    }

    pub fn init(self: &Rc<Self>, app: &Rc<App>) {
        self.connect_task_events(app);
    }

    fn connect_task_events(self: &Rc<Self>, app: &Rc<App>) {
        let self_clone = self.clone();
        let app_clone = app.clone();

        app.task_manager
            .listen(move |task_event| match &task_event.status {
                TaskStatus::Started => self_clone.add_task(&app_clone, task_event),

                TaskStatus::Finished { results } => {
                    self_clone.set_task_results(&task_event.run_id, results);
                }

                TaskStatus::Failed { error } => {
                    self_clone.set_task_error(&task_event.run_id, error);
                }

                TaskStatus::Progress {
                    action,
                    action_nr,
                    total_actions,
                    progress: _,
                    status: _,
                } => self_clone.set_task_progress(
                    &task_event.run_id,
                    action.as_deref(),
                    *action_nr,
                    *total_actions,
                ),
            });
    }

    fn add_task(self: &Rc<Self>, app: &Rc<App>, task_event: &TaskEvent) {
        let task_ui = TaskUi::from_event(task_event, &self.nav_view, app);
        let task_ui_run_id = task_ui.run_id.clone();
        self.tasks.borrow_mut().push(task_ui);

        // Reset task list so new tasks are added on top
        for task in self.tasks.borrow().iter().rev() {
            if task.run_id != task_ui_run_id {
                self.tasks_pref_group.remove(&task.task_row);
            }
            self.tasks_pref_group.add(&task.task_row);
        }
    }

    fn set_task_progress(
        self: &Rc<Self>,
        id: &str,
        action: Option<&str>,
        action_nr: Option<i32>,
        total_actions: i32,
    ) {
        let mut tasks_borrow_mut = self.tasks.borrow_mut();
        let Some(task) = tasks_borrow_mut.find_task_mut(id) else {
            error!("Failed to get ui task by id for progress");
            return;
        };

        task.set_progress(action, action_nr, total_actions);
    }

    fn set_task_results(self: &Rc<Self>, id: &str, results: &[ActionResult]) {
        let mut tasks_borrow_mut = self.tasks.borrow_mut();
        let Some(task) = tasks_borrow_mut.find_task_mut(id) else {
            error!("Failed to get ui task by id for result");
            return;
        };
        task.set_success(results);
    }

    fn set_task_error(self: &Rc<Self>, id: &str, error: &anyhow::Error) {
        let mut tasks_borrow_mut = self.tasks.borrow_mut();
        let Some(task) = tasks_borrow_mut.find_task_mut(id) else {
            error!("Failed to get ui task by id for error");
            return;
        };
        task.set_error(error);
    }
}
