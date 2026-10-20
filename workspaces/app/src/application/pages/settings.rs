mod switch_row;

use super::settings_yaml::{Link, Setting, SettingsPageYaml};
use crate::application::{
    pages::{DynPage, NavPage, PrefNavPageBuild},
    task_manager::{TaskManager, user_execution_context::UserExecutionContext},
};
use anyhow::Result;
use gtk::{Button, glib, prelude::ButtonExt};
use libadwaita::{
    ButtonContent, EntryRow, NavigationPage, PreferencesGroup, PreferencesPage,
    prelude::{PreferencesGroupExt, PreferencesPageExt},
};
use std::{process::Command, rc::Rc, sync::Arc};
use switch_row::build_switch_row;
use tracing::{debug, error};

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
        self.yaml.icon.as_deref()
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
        for group in &self.yaml.groups {
            let pref_group = PreferencesGroup::builder().build();

            if let Some(group_title) = &group.title {
                pref_group.set_title(&glib::markup_escape_text(group_title));
                if let Some(group_description) = &group.description {
                    pref_group.set_description(Some(&glib::markup_escape_text(group_description)));
                }
            }

            if let Some(group_link) = &group.link {
                let link_button = Self::create_button_from_link(group_link);
                pref_group.set_header_suffix(Some(&link_button));
            }

            for setting in &group.settings {
                match setting {
                    Setting::Input(input) => {
                        let entry_row = EntryRow::builder()
                            .title(&input.title)
                            .use_markup(false)
                            // .text()
                            .show_apply_button(true)
                            .input_purpose(input.input_type.to_gtk())
                            .build();

                        pref_group.add(&entry_row);
                    }

                    Setting::Switch(switch) => {
                        let switch_row = build_switch_row(switch, task_manager, user_context);
                        pref_group.add(&switch_row);
                    }
                }
            }

            self.prefs_page.add(&pref_group);
        }
    }

    fn create_button_from_link(group_link: &Link) -> Button {
        let button_content = ButtonContent::builder()
            .label(&group_link.title)
            .icon_name("web-browser-symbolic")
            .build();

        let link_button = Button::builder()
            .child(&button_content)
            .css_classes(["flat"])
            .build();
        let url_str = group_link.url.to_string();

        link_button.connect_clicked(move |_button| {
            debug!(url_str, "Running xdg-open");

            if let Err(error) = Command::new("xdg-open").arg(&url_str).spawn() {
                error!(%error);
            }
        });

        link_button
    }
}
