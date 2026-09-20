use super::settings_yaml::{Setting, SettingsPageYaml};
use crate::application::{
    pages::{DynPage, NavPage, Page, PrefNavPageBuild},
    task_manager::{
        TaskEvent, TaskManager, TaskStatus, action_runner::ActionRunner, actions::ActionState,
        user_execution_context::UserExecutionContext,
    },
};
use anyhow::Result;
use gtk::prelude::WidgetExt;
use libadwaita::{
    EntryRow, NavigationPage, PreferencesGroup, PreferencesPage, SwitchRow,
    prelude::{ActionRowExt, PreferencesGroupExt, PreferencesPageExt},
};
use std::rc::Rc;

pub struct SettingsPage {
    yaml: SettingsPageYaml,
    nav_page: NavigationPage,
    prefs_page: PreferencesPage,
}
impl DynPage for SettingsPage {
    fn build_page(
        self,
        task_manager: &Rc<TaskManager>,
        user_context: &UserExecutionContext,
    ) -> Result<Page> {
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

    fn build(&self, task_manager: &Rc<TaskManager>, user_context: &UserExecutionContext) {
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
                        let action_state = switch.get_status();

                        let switch_row = SwitchRow::builder()
                            .title(&switch.title)
                            .active(matches!(action_state, ActionState::Done))
                            // .sensitive(matches!(action_state, ActionState::Available))
                            .build();
                        if let Some(subtitle) = &switch.subtitle {
                            switch_row.set_subtitle(subtitle);
                        }

                        let action_runner =
                            ActionRunner::new(&switch.title, &switch.actions, user_context);
                        let action_runner_undo = action_runner.to_undo();

                        let task_manager_clone = task_manager.clone();
                        let handle_task_event =
                            |event: &TaskEvent, switch_row: &SwitchRow| match event.status {
                                TaskStatus::Finished { .. } | TaskStatus::Failed { .. } => {
                                    switch_row.set_sensitive(true);
                                }
                                _ => {}
                            };

                        switch_row.connect_active_notify(move |switch_row| {
                            switch_row.set_sensitive(false);
                            let switch_row_clone = switch_row.clone();

                            if switch_row.is_active() {
                                let _ = task_manager_clone.add(&action_runner, move |event| {
                                    handle_task_event(event, &switch_row_clone);
                                });
                            } else {
                                let _ = task_manager_clone.add(&action_runner_undo, move |event| {
                                    handle_task_event(event, &switch_row_clone);
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
