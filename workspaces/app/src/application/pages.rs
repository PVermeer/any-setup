mod content;
mod fallback;
mod page_config;
mod settings;

use crate::application::{
    App,
    pages::{fallback::FallbackPage, page_config::PageYaml},
};
use common::{app_dirs::AppDirs, utils};
use libadwaita::{
    ActionRow, HeaderBar, NavigationPage, NavigationSplitView, NavigationView, PreferencesPage,
    ToolbarView,
    gtk::{Image, prelude::WidgetExt},
    prelude::ActionRowExt,
};
use std::rc::Rc;
use tracing::error;

pub type Page = Rc<dyn DynPage>;

pub struct Pages {
    pages: Vec<Page>,
}
impl Pages {
    pub fn new(app_dirs: &Rc<AppDirs>) -> Self {
        let pages = Self::load_page_configs(app_dirs);

        Self { pages }
    }

    pub fn init(&self, app: &Rc<App>) {
        let sidebar = &app.window.view.sidebar;

        for page in &self.pages {
            sidebar.add_nav_row(app, page);
        }
    }

    pub fn get_first(&self) -> Option<&Page> {
        self.pages.first()
    }

    fn load_page_configs(app_dirs: &Rc<AppDirs>) -> Vec<Page> {
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

                let page = page_yaml.into_page();
                pages.push(page);
            }
        }

        if pages.is_empty() {
            pages.push(FallbackPage::new().build_page());
        }

        pages
    }
}

pub struct NavPageBuild {
    nav_page: NavigationPage,
    nav_row: ActionRow,
    toolbar: ToolbarView,
}
pub struct PrefNavPageBuild {
    nav_page: NavigationPage,
    nav_row: ActionRow,
    nav_view: NavigationView,
    prefs_page: PreferencesPage,
}

pub trait NavPage {
    fn get_navpage(&self) -> &NavigationPage;

    fn get_nav_row(&self) -> &ActionRow;

    fn load_page(&self, view: &NavigationSplitView) {
        let nav_page = self.get_navpage();
        if nav_page.parent().is_some() {
            return;
        }
        view.set_content(Some(nav_page));
    }

    fn build_nav_page(title: &str, icon: &str) -> NavPageBuild
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

        let nav_row = ActionRow::builder().activatable(true).title(title).build();
        let icon_prefix = Image::from_icon_name(icon);
        nav_row.add_prefix(&icon_prefix);

        NavPageBuild {
            nav_page,
            nav_row,
            toolbar,
        }
    }

    fn build_preferences_nav_page(title: &str, icon: &str) -> PrefNavPageBuild
    where
        Self: Sized,
    {
        let NavPageBuild {
            nav_page,
            nav_row,
            toolbar,
        } = Self::build_nav_page(title, icon);

        let nav_view = NavigationView::new();
        let prefs_page = PreferencesPage::new();
        let nav_view_page = NavigationPage::builder().child(&nav_view).build();
        toolbar.set_content(Some(&prefs_page));
        nav_view.add(&nav_page);

        PrefNavPageBuild {
            nav_page: nav_view_page,
            nav_row,
            nav_view,
            prefs_page,
        }
    }
}

pub trait DynPage: NavPage {
    fn build_page(self) -> Page;
}
