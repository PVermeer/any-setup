mod content;
mod fallback;
mod page_config;
mod settings;

use super::task_manager::TaskManager;
use crate::application::{
    App,
    pages::{fallback::FallbackPage, page_config::PageYaml},
};
use common::{app_dirs::AppDirs, utils};
use libadwaita::{
    HeaderBar, NavigationPage, NavigationSplitView, NavigationView, PreferencesPage, ToolbarView,
    gtk::prelude::WidgetExt,
};
use std::rc::Rc;
use tracing::error;

pub type Page = Rc<dyn DynPage>;

pub struct Pages {
    pages: Vec<Page>,
}
impl Pages {
    pub fn new(app_dirs: &Rc<AppDirs>, task_manager: &Rc<TaskManager>) -> Self {
        let pages = Self::load_page_configs(app_dirs, task_manager);

        Self { pages }
    }

    pub fn init(&self, app: &Rc<App>) {
        let sidebar = &app.window.view.sidebar;

        for page in &self.pages {
            sidebar.add_page(page);
        }
    }

    pub fn get_first(&self) -> Option<&Page> {
        self.pages.first()
    }

    fn load_page_configs(app_dirs: &Rc<AppDirs>, task_manager: &Rc<TaskManager>) -> Vec<Page> {
        let mut pages: Vec<Page> = Vec::new();

        if let Some(pages_dir) = &app_dirs.system_data_pages_dir
            && let Ok(mut pages_dir_entries) = utils::files::get_entries_in_dir(pages_dir)
        {
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

                let page = page_yaml.into_page(task_manager);
                pages.push(page);
            }
        }

        if pages.is_empty() {
            pages.push(FallbackPage::new().build_page(task_manager));
        }

        pages
    }
}

pub struct NavPageBuild {
    pub nav_page: NavigationPage,
    pub toolbar: ToolbarView,
}
pub struct PrefNavPageBuild {
    nav_page: NavigationPage,
    nav_view: NavigationView,
    prefs_page: PreferencesPage,
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
}

pub trait DynPage: NavPage {
    fn build_page(self, task_manager: &Rc<TaskManager>) -> Page;
}
