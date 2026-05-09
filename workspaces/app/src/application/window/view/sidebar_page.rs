use super::{NavPage, Page};
use crate::application::App;
use common::{
    config::{self},
    utils::{self, OnceLockExt},
};
use gtk::gio::prelude::ListModelExtManual;
use libadwaita::{
    HeaderBar, NavigationPage, Sidebar, SidebarItem, SidebarSection, ToolbarView,
    prelude::{NavigationPageExt, SidebarItemExt},
};
use std::{
    cell::{OnceCell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};
use tracing::error;

pub struct SidebarPage {
    pub nav_page: NavigationPage,
    pub header: HeaderBar,
    pages: Rc<RefCell<HashMap<SidebarItem, Page>>>,
    sections: RefCell<HashSet<SidebarSection>>,
    base_section: SidebarSection,
    sidebar: Sidebar,
    is_connected: OnceCell<bool>,
}
impl NavPage for SidebarPage {
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
impl SidebarPage {
    pub fn new() -> Self {
        let sidebar = Sidebar::builder().build();
        let base_section = SidebarSection::new();
        sidebar.append(base_section.clone());

        let header = HeaderBar::new();
        let toolbar = ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&sidebar));

        let nav_page = NavigationPage::builder()
            .title(utils::strings::capitalize(config::APP_NAME.get_value()))
            .tag("sidebar")
            .child(&toolbar)
            .build();

        Self {
            nav_page,
            header,
            pages: Rc::new(RefCell::new(HashMap::new())),
            sections: RefCell::new(HashSet::new()),
            base_section,
            sidebar,
            is_connected: OnceCell::new(),
        }
    }

    fn connect_sidebar(&self, app: &Rc<App>) {
        if self.is_connected.get().is_none() {
            let app_clone = app.clone();
            let pages_clone = self.pages.clone();

            let load_page = move |sidebar: &Sidebar| {
                let Some(selected_item) = sidebar.selected_item() else {
                    return;
                };
                let pages_borrow = pages_clone.borrow();
                let Some(page) = pages_borrow.get(&selected_item) else {
                    return;
                };
                page.load_page(&app_clone.window.view.nav_split);
            };
            load_page(&self.sidebar); // Make sure it also runs at init
            self.sidebar.connect_selected_item_notify(load_page);

            let _ = self.is_connected.set(true);
        }
    }

    pub fn add_page(&self, app: &Rc<App>, page: &Page) {
        let item = SidebarItem::builder()
            .title(page.get_navpage().title())
            .build();
        item.set_icon_name(page.get_icon());

        self.pages.borrow_mut().insert(item.clone(), page.clone());

        if let Some(section_name) = page.get_section() {
            let section = SidebarSection::new();
            section.set_title(Some(section_name));
            section.append(item);

            if self.sections.borrow_mut().insert(section.clone()) {
                self.sidebar.append(section);
            }
        } else {
            self.base_section.append(item);
        }

        self.connect_sidebar(app);
    }

    pub fn select_page(&self, page: &Page) {
        let pages_borrow = self.pages.borrow();

        let item_index = self
            .sidebar
            .items()
            .iter::<SidebarItem>()
            .position(move |sidebar_item| {
                let Ok(item) = sidebar_item else {
                    return false;
                };
                let Some(page_cached) = pages_borrow.get(&item) else {
                    return false;
                };

                Rc::ptr_eq(page_cached, page)
            })
            .and_then(|index| index.try_into().ok());

        match item_index {
            Some(index) => self.sidebar.set_selected(index),
            None => {
                error!(
                    page = &page.get_navpage().title().to_string(),
                    "Failed to select page"
                );
            }
        }
    }
}
