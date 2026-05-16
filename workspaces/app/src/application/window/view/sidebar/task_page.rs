use super::NavPage;
use crate::application::{
    App,
    pages::PrefNavPageBuild,
    task_manager::{TaskEvent, TaskStatus, action_runner::ActionResult},
};
use gtk::{
    Align, IconSize, Image, Label, Orientation,
    prelude::{BoxExt, WidgetExt},
};
use libadwaita::{
    NavigationPage, PreferencesGroup, PreferencesRow, Spinner,
    prelude::{PreferencesGroupExt, PreferencesPageExt},
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tracing::error;

#[derive(Debug)]
struct TaskUi {
    task_row: PreferencesRow,
    run_id: String,
    success_icon: Image,
    fail_icon: Image,
    running_icon: Spinner,
    results: Option<Vec<ActionResult>>,
    error: Option<String>,
}
impl TaskUi {
    fn from_event(task_event: &TaskEvent) -> Self {
        let task_event = task_event.clone();
        let title = &task_event.name;
        let (task_row, success_icon, fail_icon, running_icon) = Self::build_task_row(title);

        Self {
            task_row,
            run_id: task_event.run_id,
            success_icon,
            fail_icon,
            running_icon,
            results: None,
            error: None,
        }
    }

    fn update(&self) {
        if self.results.is_some() {
            self.success_icon.set_visible(true);
            self.fail_icon.set_visible(false);
            self.running_icon.set_visible(false);
        }
        if self.error.is_some() {
            self.success_icon.set_visible(false);
            self.fail_icon.set_visible(true);
            self.running_icon.set_visible(false);
            self.task_row.add_css_class("error");
        }
    }

    fn build_task_row(title: &str) -> (PreferencesRow, Image, Image, Spinner) {
        let margin = 12;

        let task_content_box = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .margin_top(margin)
            .margin_bottom(margin)
            .margin_start(margin)
            .margin_end(margin)
            .spacing(margin)
            .height_request(70)
            .build();

        let task_icon = Image::builder()
            .icon_name("system-run-symbolic")
            .icon_size(IconSize::Large)
            .build();
        task_content_box.append(&task_icon);

        let task_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(margin)
            .build();

        let title = Label::builder().label(title).build();
        task_box.append(&title);

        let task_status_box = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(margin)
            .halign(Align::End)
            .hexpand(true)
            .build();

        let success_icon = Image::builder()
            .icon_name("object-select-symbolic")
            .css_classes(["success"])
            .icon_size(IconSize::Large)
            .visible(false)
            .build();
        let fail_icon = Image::builder()
            .icon_name("process-stop-symbolic")
            .css_classes(["error"])
            .icon_size(IconSize::Large)
            .visible(false)
            .build();
        let running_icon = Spinner::builder()
            .height_request(30)
            .width_request(30)
            .build();

        task_status_box.append(&success_icon);
        task_status_box.append(&fail_icon);
        task_status_box.append(&running_icon);

        task_content_box.append(&task_box);
        task_content_box.append(&task_status_box);

        let task_row = PreferencesRow::builder().child(&task_content_box).build();

        (task_row, success_icon, fail_icon, running_icon)
    }
}

pub struct TaskPage {
    nav_page: NavigationPage,
    tasks_pref_group: PreferencesGroup,
    tasks: RefCell<HashMap<String, TaskUi>>,
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
            prefs_page,
            ..
        } = Self::build_preferences_nav_page(&t!("pages.tasks.title"));

        let tasks_pref_group = PreferencesGroup::builder()
            .title(t!("pages.tasks.title"))
            .build();

        prefs_page.add(&tasks_pref_group);

        Rc::new(Self {
            nav_page,
            tasks_pref_group,
            tasks: RefCell::new(HashMap::new()),
        })
    }

    pub fn init(self: &Rc<Self>, app: &Rc<App>) {
        self.connect_task_events(app);
    }

    fn connect_task_events(self: &Rc<Self>, app: &Rc<App>) {
        let self_clone = self.clone();

        app.task_manager
            .listen(move |task_event| match &task_event.status {
                TaskStatus::Started => self_clone.add_task(task_event),

                TaskStatus::Finished { results } => {
                    self_clone.set_task_results(&task_event.run_id, results);
                }

                TaskStatus::Failed { error } => {
                    self_clone.set_task_error(&task_event.run_id, error);
                }

                _ => {}
            });
    }

    fn add_task(self: &Rc<Self>, task_event: &TaskEvent) {
        let task_ui = TaskUi::from_event(task_event);
        self.tasks_pref_group.add(&task_ui.task_row);

        let replaced_task = self
            .tasks
            .borrow_mut()
            .insert(task_ui.run_id.clone(), task_ui);

        if replaced_task.is_some() {
            error!(
                ?task_event,
                "Task already added to the ui, this should not happen!"
            );
        }
    }

    fn set_task_results(self: &Rc<Self>, id: &str, results: &[ActionResult]) {
        let mut tasks_borrow_mut = self.tasks.borrow_mut();
        let Some(task) = tasks_borrow_mut.get_mut(id) else {
            error!("Failed to get ui task by id to set error");
            return;
        };
        task.results = Some(results.to_owned());
        task.update();
    }

    fn set_task_error(self: &Rc<Self>, id: &str, error: &anyhow::Error) {
        let mut tasks_borrow_mut = self.tasks.borrow_mut();
        let Some(task) = tasks_borrow_mut.get_mut(id) else {
            error!("Failed to get ui task by id to set error");
            return;
        };
        task.error = Some(error.to_string());
        task.update();
    }
}
