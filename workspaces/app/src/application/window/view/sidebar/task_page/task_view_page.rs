use super::task_progress::TaskProgress;
use crate::application::{
    App,
    pages::{NavPage, PrefNavPageBuild},
    task_manager::{TaskEvent, TaskStatus, action_runner::ActionResult},
};
use gtk::{
    Image, TextBuffer, TextView, WrapMode,
    prelude::{TextBufferExt, TextBufferExtManual, TextTagExt, TextViewExt, WidgetExt},
};
use libadwaita::{
    ActionRow, NavigationPage, PreferencesGroup, PreferencesPage, PreferencesRow, Spinner,
    prelude::{ActionRowExt, PreferencesGroupExt, PreferencesPageExt, PreferencesRowExt},
};
use std::{fmt::Display, rc::Rc};

enum TextBufferTag {
    Error,
    Success,
}
impl Display for TextBufferTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Success => write!(f, "succes"),
        }
    }
}
impl TextBufferTag {
    fn add_all_to_text_buffer(text_buffer: &TextBuffer) {
        text_buffer
            .create_tag(Some(&Self::Error.to_string()), &[])
            .unwrap_or_default()
            .set_foreground(Some("red"));

        text_buffer
            .create_tag(Some(&Self::Success.to_string()), &[])
            .unwrap_or_default()
            .set_foreground(Some("green"));
    }
}

pub struct TaskViewPage {
    run_id: String,
    nav_page: NavigationPage,
    prefs_page: PreferencesPage,
    status_prefs_group: PreferencesGroup,
    task_progress: TaskProgress,
    status_row: ActionRow,
    status_running_icon: Spinner,
    status_success_icon: Image,
    status_fail_icon: Image,
    task_output: TextView,
}
impl NavPage for TaskViewPage {
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
impl TaskViewPage {
    pub fn from_event(task_event: &TaskEvent) -> Rc<Self> {
        let title = task_event.name.clone();
        let id = task_event.run_id.clone();

        let PrefNavPageBuild {
            nav_page,
            prefs_page,
            ..
        } = Self::build_preferences_nav_page(&title);

        let progress_prefs_group = PreferencesGroup::new();
        prefs_page.add(&progress_prefs_group);

        let task_progress = TaskProgress::new(Some(&id));
        progress_prefs_group.add(task_progress.get_progress_bar());

        let status_prefs_group = PreferencesGroup::builder()
            .title(t!("pages.tasks.details.status.title"))
            .build();
        prefs_page.add(&status_prefs_group);

        let (status_row, status_running_icon, status_success_icon, status_fail_icon) =
            Self::build_status_row();
        status_prefs_group.add(&status_row);

        let (task_output_row, task_output) = Self::build_output_row();
        status_prefs_group.add(&task_output_row);

        Rc::new(Self {
            run_id: id,
            nav_page,
            prefs_page,
            status_prefs_group,
            task_progress,
            status_row,
            status_running_icon,
            status_success_icon,
            status_fail_icon,
            task_output,
        })
    }

    pub fn init(self: Rc<Self>, app: &Rc<App>) -> Rc<Self> {
        self.task_progress.init(app);
        self.connect_task_events(app);

        self
    }

    fn build_status_row() -> (ActionRow, Spinner, Image, Image) {
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

        let row = ActionRow::builder().build();
        row.add_suffix(&running_icon);
        row.add_suffix(&success_icon);
        row.add_suffix(&fail_icon);

        (row, running_icon, success_icon, fail_icon)
    }

    fn build_output_row() -> (PreferencesRow, TextView) {
        let task_output = TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(WrapMode::Word)
            .indent(-25)
            .height_request(150)
            .build();
        let task_output_buffer = task_output.buffer();

        TextBufferTag::add_all_to_text_buffer(&task_output_buffer);

        // let task_status = StatusPage::builder().child(&task_output).build();
        let margin = 12;
        let task_output_row = PreferencesRow::builder()
            .child(&task_output)
            .activatable(false)
            .margin_top(margin)
            .margin_bottom(margin)
            .margin_start(margin)
            .margin_end(margin)
            .height_request(100)
            .build();

        (task_output_row, task_output)
    }

    fn connect_task_events(self: &Rc<Self>, app: &Rc<App>) {
        let self_clone = self.clone();
        let run_id = self.run_id.clone();

        app.task_manager.listen(move |task_event| {
            if task_event.run_id != run_id {
                return;
            }

            match &task_event.status {
                TaskStatus::Started => {} // Self is created from start event

                TaskStatus::Finished { results } => {
                    self_clone.set_success(results);
                }

                TaskStatus::Failed { error } => {
                    self_clone.set_error(error);
                }

                TaskStatus::Progress {
                    action,
                    action_nr,
                    total_actions,
                    progress: _,
                    status: _,
                } => self_clone.set_progress(
                    &task_event.run_id,
                    action.as_deref(),
                    *action_nr,
                    *total_actions,
                ),
            }
        });
    }

    fn set_progress(
        self: &Rc<Self>,
        id: &str,
        action: Option<&str>,
        action_nr: Option<i32>,
        total_actions: i32,
    ) {
        let Some(action) = action else {
            self.status_row.set_title("");
            return;
        };

        let progress = if let Some(task_nr) = action_nr {
            format!("{task_nr} / {total_actions} : {action}")
        } else {
            String::new()
        };

        self.status_row.set_subtitle(&progress);
        self.output_append_line(action);
    }

    fn set_success(self: &Rc<Self>, results: &[ActionResult]) {
        self.status_row
            .set_subtitle(&t!("pages.tasks.details.status.success"));

        self.status_success_icon.set_visible(true);
        self.status_fail_icon.set_visible(false);
        self.status_running_icon.set_visible(false);

        self.output_append_success(&t!("pages.tasks.details.status.success"));
        for result in results {
            if result.stdout.is_empty() {
                continue;
            }
            self.output_append_line(&result.stdout);
        }
    }

    fn set_error(self: &Rc<Self>, error: &anyhow::Error) {
        self.status_row
            .set_subtitle(&t!("pages.tasks.details.status.error"));
        self.status_row.add_css_class("error");

        self.status_success_icon.set_visible(false);
        self.status_fail_icon.set_visible(true);
        self.status_running_icon.set_visible(false);

        self.output_append_error(&error.to_string());
    }

    fn output_append(self: &Rc<Self>, line: &str, tag: Option<TextBufferTag>) {
        let text_buffer = self.task_output.buffer();
        let mut end_iter = text_buffer.end_iter();

        match tag {
            None => text_buffer.insert(&mut end_iter, &format!("{line}\n")),
            Some(tag) => text_buffer.insert_with_tags_by_name(
                &mut end_iter,
                &format!("{line}\n"),
                &[&tag.to_string()],
            ),
        }
    }

    fn output_append_line(self: &Rc<Self>, line: &str) {
        self.output_append(line, None);
    }

    fn output_append_error(self: &Rc<Self>, line: &str) {
        self.output_append(&format!("✗ {line}"), Some(TextBufferTag::Error));
    }

    fn output_append_success(self: &Rc<Self>, line: &str) {
        self.output_append(&format!("✓ {line}"), Some(TextBufferTag::Success));
    }
}
