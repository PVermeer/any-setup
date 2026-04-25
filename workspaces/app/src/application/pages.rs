mod content;
mod fallback;
mod page_config;

use crate::application::App;
use common::{app_dirs::AppDirs, utils};
use content::ContentPage;
use fallback::FallbackPage;
use libadwaita::{
    ActionRow, HeaderBar, NavigationPage, NavigationSplitView, ToolbarView,
    gtk::{Image, prelude::WidgetExt},
    prelude::ActionRowExt,
};
use page_config::{PageType, PageYaml};
use std::rc::Rc;
use tracing::error;

pub type Page = Rc<dyn NavPage>;

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
                let page = match page_yaml.page_type {
                    PageType::Content => ContentPage::from_yaml(&page_yaml),
                };

                pages.push(page);
            }
        }

        if pages.is_empty() {
            pages.push(FallbackPage::new());
        }

        pages
    }
}

pub struct NavPageBuild {
    nav_page: NavigationPage,
    nav_row: ActionRow,
    toolbar: ToolbarView,
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
}

// struct PrefPage {
//     nav_page: NavigationPage,
//     nav_row: ActionRow,
//     prefs_page: PreferencesPage,
//     toast_overlay: ToastOverlay,
// }
// struct PrefNavPage {
//     nav_page: NavigationPage,
//     nav_row: ActionRow,
//     nav_view: NavigationView,
//     prefs_page: PreferencesPage,
// }
// pub struct PageBuilder {
//     nav_page: NavigationPage,
//     nav_row: ActionRow,
//     toolbar: ToolbarView,
// }
// impl PageBuilder {
//     fn with_content_box(self) -> ContentPage {
//         const MARGIN: i32 = 20;
//         const MAX_WIDTH: i32 = 600;

//         let content_box = gtk::Box::builder()
//             .orientation(Orientation::Vertical)
//             .margin_top(MARGIN)
//             .margin_bottom(MARGIN)
//             .margin_start(MARGIN)
//             .margin_end(MARGIN)
//             .build();
//         let clamp = Clamp::builder()
//             .maximum_size(MAX_WIDTH)
//             .child(&content_box)
//             .build();
//         let scrolled_window = ScrolledWindow::builder().child(&clamp).build();
//         self.toolbar.set_content(Some(&scrolled_window));

//         ContentPage {
//             nav_page: self.nav_page,
//             nav_row: self.nav_row,
//             content_box,
//         }
//     }

// fn with_preference_page(self) -> PrefPage {
//     let prefs_page = PreferencesPage::new();
//     let toast_overlay = ToastOverlay::new();
//     toast_overlay.set_child(Some(&prefs_page));
//     self.toolbar.set_content(Some(&toast_overlay));

//     PrefPage {
//         nav_page: self.nav_page,
//         nav_row: self.nav_row,
//         prefs_page,
//         toast_overlay,
//         header: self.header,
//     }
// }

// /// This has a `NavigationView` for animations deeper in settings
// fn with_preference_navigation_view(self) -> PrefNavPage {
//     let nav_view = NavigationView::new();
//     let prefs_page = PreferencesPage::new();
//     let nav_view_page = NavigationPage::builder().child(&nav_view).build();
//     self.toolbar.set_content(Some(&prefs_page));
//     nav_view.add(&self.nav_page);

//     PrefNavPage {
//         nav_page: nav_view_page,
//         nav_row: self.nav_row,
//         nav_view,
//         prefs_page,
//     }
// }
// }
