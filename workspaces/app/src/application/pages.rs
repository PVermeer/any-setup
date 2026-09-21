mod content;
mod content_yaml;
mod fallback;
pub mod page_config;
mod settings;
mod settings_yaml;

use super::task_manager::{
    TaskManager, action_runner::ActionRunner, user_execution_context::UserExecutionContext,
};
use crate::application::{
    App,
    pages::{fallback::FallbackPage, page_config::PageYaml},
};
use anyhow::{Context, Result};
use common::{app_dirs::AppDirs, utils};
use gtk::{Orientation, ScrolledWindow};
use libadwaita::{
    Clamp, HeaderBar, NavigationPage, NavigationSplitView, NavigationView, PreferencesPage,
    ToolbarView, gtk::prelude::WidgetExt,
};
use std::{collections::HashMap, rc::Rc, sync::Arc};
use tracing::{debug, error};

pub struct Pages {
    pub pages: Vec<Rc<dyn DynPage>>,
}
impl Pages {
    pub fn new(
        app_dirs: &Rc<AppDirs>,
        task_manager: &Rc<TaskManager>,
        user_context: &UserExecutionContext,
    ) -> Result<Self> {
        let pages = Self::load_page_configs(app_dirs, task_manager, user_context)?;

        Ok(Self { pages })
    }

    pub fn init(&self, app: &Rc<App>) {
        let sidebar = &app.window.view.sidebar;

        for page in &self.pages {
            let nav_page: Rc<dyn NavPage> = page.clone();
            sidebar.add_page(&nav_page.clone());
        }
    }

    pub fn get_first(&self) -> Option<&Rc<dyn DynPage>> {
        self.pages.first()
    }

    fn load_page_configs(
        app_dirs: &Rc<AppDirs>,
        task_manager: &Rc<TaskManager>,
        user_context: &UserExecutionContext,
    ) -> Result<Vec<Rc<dyn DynPage>>> {
        let mut pages: Vec<Rc<dyn DynPage>> = Vec::new();

        if let Some(pages_dir) = &app_dirs.system_data_pages_dir
            && let Ok(mut pages_dir_entries) = utils::files::get_entries_in_dir(pages_dir)
        {
            debug!(?pages_dir, "Loading page files");

            pages_dir_entries.sort_by_key(std::fs::DirEntry::file_name);

            for dir_entry in pages_dir_entries {
                let path = dir_entry.path();

                if path
                    .extension()
                    .is_none_or(|extension| extension != "yml" && extension != "yaml")
                {
                    continue;
                }

                let page_yaml = match PageYaml::from_file(&path) {
                    Ok(page_yaml) => page_yaml,
                    Err(error) => {
                        error!(?error);
                        continue;
                    }
                };

                let page = match page_yaml
                    .into_page(task_manager, user_context)
                    .context("Failed to create page from yaml")
                {
                    Ok(page) => page,
                    Err(error) => {
                        error!(?error);
                        continue;
                    }
                };

                pages.push(page);
            }
        }

        if pages.is_empty() {
            pages.push(FallbackPage::new().build_page(task_manager, user_context)?);
        }

        Ok(pages)
    }
}

pub struct NavPageBuild {
    pub nav_page: NavigationPage,
    pub toolbar: ToolbarView,
}
pub struct PrefNavPageBuild {
    pub nav_page: NavigationPage,
    pub nav_view: NavigationView,
    pub prefs_page: PreferencesPage,
}
pub struct ContentNavPageBuild {
    pub nav_page: NavigationPage,
    pub _toolbar: ToolbarView,
    pub content: gtk::Box,
}

pub trait NavPage {
    fn get_navpage(&self) -> &NavigationPage;

    fn get_section(&self) -> Option<&str>;

    fn get_icon(&self) -> Option<&str>;

    fn load_page(&self, view: &NavigationSplitView) {
        let nav_page = self.get_navpage();
        if nav_page.parent().is_some() {
            return;
        }
        view.set_content(Some(nav_page));
    }

    fn build_nav_page(title: &str) -> NavPageBuild
    where
        Self: Sized,
    {
        let header = HeaderBar::new();
        let toolbar = ToolbarView::new();
        toolbar.add_top_bar(&header);

        let nav_page = NavigationPage::builder()
            .title(title)
            .tag(title)
            .child(&toolbar)
            .build();

        NavPageBuild { nav_page, toolbar }
    }

    fn build_preferences_nav_page(title: &str) -> PrefNavPageBuild
    where
        Self: Sized,
    {
        let NavPageBuild { nav_page, toolbar } = Self::build_nav_page(title);

        let nav_view = NavigationView::new();
        let prefs_page = PreferencesPage::new();
        let nav_view_page = NavigationPage::builder()
            .title(title)
            .child(&nav_view)
            .build();
        toolbar.set_content(Some(&prefs_page));
        nav_view.add(&nav_page);

        PrefNavPageBuild {
            nav_page: nav_view_page,
            nav_view,
            prefs_page,
        }
    }

    fn build_content_nav_page(title: &str) -> ContentNavPageBuild
    where
        Self: Sized,
    {
        let NavPageBuild { nav_page, toolbar } = Self::build_nav_page(title);
        let spacing = 20;
        let max_width = 600;

        let content_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .margin_top(spacing)
            .margin_bottom(spacing)
            .margin_start(spacing)
            .margin_end(spacing)
            .spacing(spacing)
            .build();
        let clamp = Clamp::builder()
            .maximum_size(max_width)
            .child(&content_box)
            .build();
        let scrolled_window = ScrolledWindow::builder().child(&clamp).build();
        toolbar.set_content(Some(&scrolled_window));

        ContentNavPageBuild {
            nav_page,
            _toolbar: toolbar,
            content: content_box,
        }
    }
}

pub trait DynPage: NavPage {
    fn build_page(
        self,
        task_manager: &Rc<TaskManager>,
        user_context: &UserExecutionContext,
    ) -> Result<Rc<dyn DynPage>>;
}

pub trait YamlPage {
    fn get_action_runners(
        &self,
        user_context: &UserExecutionContext,
    ) -> Result<HashMap<u64, Arc<ActionRunner>>>;
}
