use crate::application::{
    action_manager::ActionManager,
    pages::{DynPage, NavPage, Page, PrefNavPageBuild},
};
use gtk::InputPurpose;
use libadwaita::{
    ActionRow, EntryRow, NavigationPage, NavigationView, PreferencesGroup, PreferencesPage,
    SwitchRow,
    prelude::{ActionRowExt, PreferencesGroupExt, PreferencesPageExt},
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

#[derive(PartialEq, Deserialize)]
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

#[derive(PartialEq, Deserialize)]
struct Input {
    title: String,
    input_type: InputType,
}

#[derive(PartialEq, Deserialize)]
struct Switch {
    title: String,
    subtitle: Option<String>,
}

#[derive(PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Setting {
    Input(Input),
    Switch(Switch),
}

#[derive(PartialEq, Deserialize)]
struct Group {
    title: Option<String>,
    settings: Vec<Setting>,
}

#[derive(PartialEq, Deserialize)]
pub struct SettingsPage {
    title: String,
    icon: String,
    groups: Vec<Group>,

    #[serde(skip)]
    nav_page: NavigationPage,
    #[serde(skip)]
    nav_row: ActionRow,
    #[serde(skip)]
    nav_view: NavigationView,
    #[serde(skip)]
    prefs_page: PreferencesPage,
}
impl DynPage for SettingsPage {
    fn build_page(mut self, action_manager: &Rc<RefCell<ActionManager>>) -> Page {
        let PrefNavPageBuild {
            nav_page,
            nav_row,
            nav_view,
            prefs_page,
        } = Self::build_preferences_nav_page(&self.title, &self.icon);
        self.nav_page = nav_page;
        self.nav_row = nav_row;
        self.nav_view = nav_view;
        self.prefs_page = prefs_page;

        self.build(action_manager);

        Rc::new(self)
    }
}
impl NavPage for SettingsPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_nav_row(&self) -> &ActionRow {
        &self.nav_row
    }
}
impl SettingsPage {
    fn build(&self, action_manager: &Rc<RefCell<ActionManager>>) {
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
                        let switch_row = SwitchRow::builder()
                            .title(&switch.title)
                            // .active()
                            .build();
                        if let Some(subtitle) = &switch.subtitle {
                            switch_row.set_subtitle(subtitle);
                        }

                        pref_group.add(&switch_row);
                    }
                }
            }

            self.prefs_page.add(&pref_group);
        }
    }
}
