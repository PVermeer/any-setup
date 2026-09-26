use super::settings_yaml::{Setting, SettingsPageYaml, Switch};
use crate::application::{
    pages::{DynPage, NavPage, PrefNavPageBuild},
    task_manager::{
        TaskEvent, TaskManager, TaskStatus, action_runner::ActionRunner,
        user_execution_context::UserExecutionContext,
    },
};
use anyhow::Result;
use gtk::prelude::WidgetExt;
use libadwaita::{
    EntryRow, NavigationPage, PreferencesGroup, PreferencesPage, Spinner, SwitchRow,
    prelude::{ActionRowExt, PreferencesGroupExt, PreferencesPageExt},
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

pub struct SettingsPage {
    yaml: SettingsPageYaml,
    nav_page: NavigationPage,
    prefs_page: PreferencesPage,
}
impl DynPage for SettingsPage {
    fn build_page(
        self,
        task_manager: &Rc<TaskManager>,
        user_context: &Arc<UserExecutionContext>,
    ) -> Result<Rc<dyn DynPage>> {
        self.build(task_manager, user_context);

        Ok(Rc::new(self))
    }
}
impl NavPage for SettingsPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_section(&self) -> Option<&str> {
        self.yaml.section.as_deref()
    }

    fn get_icon(&self) -> Option<&str> {
        Some(&self.yaml.icon)
    }
}
impl SettingsPage {
    pub fn new(yaml: SettingsPageYaml) -> Self {
        let PrefNavPageBuild {
            nav_page,
            nav_view: _,
            prefs_page,
        } = Self::build_preferences_nav_page(&yaml.title);

        Self {
            yaml,
            nav_page,
            prefs_page,
        }
    }

    fn build(&self, task_manager: &Rc<TaskManager>, user_context: &Arc<UserExecutionContext>) {
        let user_context = user_context.clone();

        for group in &self.yaml.groups {
            let pref_group = PreferencesGroup::builder().build();

            if let Some(group_title) = &group.title {
                pref_group.set_title(group_title);
            }

            for setting in &group.settings {
                match setting {
                    Setting::Input(input) => {
                        let entry_row = EntryRow::builder()
                            .title(&input.title)
                            // .text()
                            .show_apply_button(true)
                            .input_purpose(input.input_type.to_gtk())
                            .build();

                        pref_group.add(&entry_row);
                    }

                    Setting::Switch(switch) => {
                        let switch = Rc::new(switch.clone());

                        let switch_row = SwitchRow::builder().title(&switch.title).build();
                        if let Some(subtitle) = &switch.subtitle {
                            switch_row.set_subtitle(subtitle);
                        }
                        switch.set_switch_row_from_status(&switch_row, &user_context);

                        if cfg!(debug_assertions) {
                            switch_row.set_sensitive(true);
                        }

                        let spinner = Spinner::new();
                        spinner.set_visible(false);
                        switch_row.add_suffix(&spinner);

                        let action_runner =
                            ActionRunner::new(&switch.title, &switch.actions, &user_context);
                        let action_runner_undo = action_runner.to_undo();

                        let handle_task_event =
                            move |event: &TaskEvent,
                             switch: &Rc<Switch>,
                             switch_row: &SwitchRow,
                             spinner: &Spinner,
                             user_context: &Arc<UserExecutionContext>,
                             disable_active_notify: &RefCell<bool>| {
                                match event.status {
                                    TaskStatus::Finished { .. } | TaskStatus::Failed { .. } => {
                                        switch_row.set_sensitive(true);
                                        spinner.set_visible(false);

                                        *disable_active_notify.borrow_mut() = true;
                                        switch.set_switch_row_from_status(switch_row, user_context);
                                        *disable_active_notify.borrow_mut() = false;
                                    }
                                    _ => {}
                                }
                            };

                        let switch_clone = switch.clone();
                        let task_manager_clone = task_manager.clone();
                        let spinner_clone = spinner.clone();
                        let user_contex_clone = user_context.clone();
                        let disable_active_notify = Rc::new(RefCell::from(false));

                        switch_row.connect_active_notify(move |switch_row| {
                            if *disable_active_notify.borrow() {
                                return;
                            }

                            switch_row.set_sensitive(false);
                            spinner_clone.set_visible(true);

                            let switch_clone = switch_clone.clone();
                            let switch_row_clone = switch_row.clone();
                            let spinner_clone = spinner_clone.clone();
                            let user_contex_clone = user_contex_clone.clone();
                            let disable_active_notify_clone = disable_active_notify.clone();

                            if switch_row.is_active() {
                                let _ = task_manager_clone.add(&action_runner, move |event| {
                                    handle_task_event(
                                        event,
                                        &switch_clone,
                                        &switch_row_clone,
                                        &spinner_clone,
                                        &user_contex_clone,
                                        &disable_active_notify_clone,
                                    );
                                });
                            } else {
                                let _ = task_manager_clone.add(&action_runner_undo, move |event| {
                                    handle_task_event(
                                        event,
                                        &switch_clone,
                                        &switch_row_clone,
                                        &spinner_clone,
                                        &user_contex_clone,
                                        &disable_active_notify_clone,
                                    );
                                });
                            }
                        });

                        pref_group.add(&switch_row);
                    }
                }
            }

            self.prefs_page.add(&pref_group);
        }
    }
}
