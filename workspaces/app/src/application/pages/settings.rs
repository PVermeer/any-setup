use crate::application::{
    pages::{DynPage, NavPage, Page, PrefNavPageBuild},
    task_manager::{
        TaskEvent, TaskManager, TaskStatus,
        action_runner::ActionRunner,
        actions::{Action, ActionState, IsAction},
    },
};
use anyhow::{Context, Result};
use gtk::{InputPurpose, prelude::WidgetExt};
use libadwaita::{
    EntryRow, NavigationPage, NavigationView, PreferencesGroup, PreferencesPage, SwitchRow,
    prelude::{ActionRowExt, PreferencesGroupExt, PreferencesPageExt},
};
use serde::Deserialize;
use std::rc::Rc;

#[derive(PartialEq, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum InputType {
    FreeForm,
    Digits,
    Number,
    Phone,
    Url,
    Email,
    Name,
    Password,
    Pin,
}
impl InputType {
    fn to_gtk(&self) -> InputPurpose {
        match self {
            Self::FreeForm => InputPurpose::FreeForm,
            Self::Digits => InputPurpose::Digits,
            Self::Number => InputPurpose::Number,
            Self::Phone => InputPurpose::Phone,
            Self::Url => InputPurpose::Url,
            Self::Email => InputPurpose::Email,
            Self::Name => InputPurpose::Name,
            Self::Password => InputPurpose::Password,
            Self::Pin => InputPurpose::Pin,
        }
    }
}

#[derive(PartialEq, Deserialize, Debug)]
struct Input {
    title: String,
    input_type: InputType,
}

#[derive(PartialEq, Deserialize, Debug)]
struct Switch {
    title: String,
    subtitle: Option<String>,
    actions: Vec<Action>,
}
impl Switch {
    fn get_status(&self) -> ActionState {
        let status: Vec<ActionState> = self
            .actions
            .iter()
            .map(|action| action.get_status().unwrap_or_default())
            .collect();

        let done = status.iter().all(|status| *status == ActionState::Done);
        let available = status
            .iter()
            .all(|status| *status != ActionState::UnAvailable);

        if done {
            return ActionState::Done;
        }
        if available {
            return ActionState::Available;
        }
        ActionState::UnAvailable
    }
}

#[derive(PartialEq, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Setting {
    Input(Input),
    Switch(Switch),
}

#[derive(PartialEq, Deserialize, Debug)]
struct Group {
    title: Option<String>,
    settings: Vec<Setting>,
}

#[derive(PartialEq, Deserialize, Debug)]
pub struct SettingsPage {
    title: String,
    section: Option<String>,
    icon: String,
    groups: Vec<Group>,

    #[serde(skip)]
    nav_page: NavigationPage,
    #[serde(skip)]
    nav_view: NavigationView,
    #[serde(skip)]
    prefs_page: PreferencesPage,
}
impl DynPage for SettingsPage {
    fn build_page(mut self, task_manager: &Rc<TaskManager>) -> Result<Page> {
        let PrefNavPageBuild {
            nav_page,
            nav_view,
            prefs_page,
        } = Self::build_preferences_nav_page(&self.title);
        self.nav_page = nav_page;
        self.nav_view = nav_view;
        self.prefs_page = prefs_page;

        self.build(task_manager)
            .context("Failed to build Setttings Page")?;

        Ok(Rc::new(self))
    }
}
impl NavPage for SettingsPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_section(&self) -> Option<&str> {
        self.section.as_deref()
    }

    fn get_icon(&self) -> Option<&str> {
        Some(&self.icon)
    }
}
impl SettingsPage {
    fn build(&self, task_manager: &Rc<TaskManager>) -> Result<()> {
        for group in &self.groups {
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
                            .active(action_state == ActionState::Done)
                            // .sensitive(action_state == ActionState::Available)
                            .build();
                        if let Some(subtitle) = &switch.subtitle {
                            switch_row.set_subtitle(subtitle);
                        }

                        let mut action_runner = ActionRunner::new(&switch.title)?;
                        let mut action_runner_undo = action_runner.clone();
                        action_runner.add_many(&switch.actions);
                        action_runner_undo
                            .add_many(&switch.actions.iter().map(IsAction::to_undo).collect());
                        action_runner_undo.set_undo(true);

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
                                let _ =
                                    task_manager_clone.add(action_runner.clone(), move |event| {
                                        handle_task_event(event, &switch_row_clone);
                                    });
                            } else {
                                let _ = task_manager_clone.add(
                                    action_runner_undo.clone(),
                                    move |event| {
                                        handle_task_event(event, &switch_row_clone);
                                    },
                                );
                            }
                        });

                        pref_group.add(&switch_row);
                    }
                }
            }

            self.prefs_page.add(&pref_group);
        }

        Ok(())
    }
}
