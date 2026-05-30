mod task_page;

use super::{NavPage, Page};
use crate::application::App;
use common::{
    config::{self},
    utils::{self, OnceLockExt},
};
use gtk::{ListBox, ListBoxRow, Orientation, gio::prelude::ListModelExtManual, prelude::BoxExt};
use libadwaita::{
    HeaderBar, NavigationPage, Sidebar, SidebarItem, SidebarMode, SidebarSection, ToolbarView,
    prelude::{NavigationPageExt, SidebarItemExt},
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};
use task_page::{TaskPage, task_progress::TaskProgress};
use tracing::error;

pub struct SidebarPage {
    pub nav_page: NavigationPage,
    pub header: HeaderBar,
    pages: Rc<RefCell<HashMap<SidebarItem, Page>>>,
    sections: RefCell<HashSet<SidebarSection>>,
    base_section: SidebarSection,
    sidebar: Sidebar,
    bottom_box: ListBox,
    task_progress: TaskProgress,
    task_page: Rc<TaskPage>,
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
        let (sidebar, base_section) = Self::build_side_bar();
        let (bottom_box, task_progress) = Self::build_bottom_box();
        let task_page = TaskPage::new();

        let layout_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .build();
        layout_box.append(&sidebar);
        layout_box.append(&bottom_box);

        let header = HeaderBar::new();
        let toolbar = ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&layout_box));

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
            bottom_box,
            task_progress,
            task_page,
        }
    }

    pub fn init(&self, app: &Rc<App>) {
        self.connect_sidebar(app);
        self.task_progress.init(app);
        self.task_page.init(app);
        self.connect_progress_button(app);
    }

    fn build_side_bar() -> (Sidebar, SidebarSection) {
        let sidebar = Sidebar::builder()
            .mode(SidebarMode::Sidebar)
            .vexpand(true)
            .build();
        let base_section = SidebarSection::new();
        sidebar.append(base_section.clone());

        (sidebar, base_section)
    }

    fn build_bottom_box() -> (ListBox, TaskProgress) {
        let task_progress = TaskProgress::new(None);

        let bottom_box = ListBox::builder()
            .css_classes(["navigation-sidebar"])
            .build();
        bottom_box.append(task_progress.get_button_row());

        (bottom_box, task_progress)
    }

    fn connect_sidebar(&self, app: &Rc<App>) {
        let app_clone = app.clone();
        let pages_clone = self.pages.clone();
        let bottom_box_clone = self.bottom_box.clone();

        let load_page = move |sidebar: &Sidebar| {
            let Some(selected_item) = sidebar.selected_item() else {
                return;
            };
            let pages_borrow = pages_clone.borrow();
            let Some(page) = pages_borrow.get(&selected_item) else {
                return;
            };
            page.load_page(&app_clone.window.view.nav_split);
            app_clone.window.view.nav_split.set_show_content(true);
            bottom_box_clone.select_row(None::<&ListBoxRow>);
        };
        load_page(&self.sidebar); // Make sure it also runs at init
        self.sidebar.connect_selected_item_notify(load_page);
    }

    fn connect_progress_button(&self, app: &Rc<App>) {
        let app_clone = app.clone();
        let sidebar_clone = self.sidebar.clone();
        let task_page_clone = self.task_page.clone();

        self.task_progress
            .get_button_row()
            .connect_activated(move |_button_row| {
                sidebar_clone.set_selected(u32::MAX); // Unselect
                task_page_clone.load_page(&app_clone.window.view.nav_split);
            });
    }

    pub fn add_page(&self, page: &Page) {
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
