use super::{NavPage, Page};
use crate::application::{
    App,
    action_manager::{TaskEvent, TaskEventEnum},
};
use common::{
    config::{self},
    utils::{self, OnceLockExt},
};
use gtk::{
    ListBox, ListBoxRow, Orientation, ProgressBar,
    gio::prelude::ListModelExtManual,
    prelude::{BoxExt, WidgetExt},
};
use libadwaita::{
    ButtonRow, HeaderBar, NavigationPage, Sidebar, SidebarItem, SidebarMode, SidebarSection,
    ToolbarView,
    prelude::{NavigationPageExt, SidebarItemExt},
};
use std::{
    cell::RefCell,
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
    bottom_box: ListBox,
    progress_button: ButtonRow,
    progress_bar: ProgressBar,
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
        let (bottom_box, progress_button, progress_bar) = Self::build_bottom_box();

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
            progress_bar,
            progress_button,
        }
    }

    pub fn init(&self, app: &Rc<App>) {
        self.connect_sidebar(app);
        self.connect_action_manager_progess(app);
        self.connect_progress_button();
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

    fn build_bottom_box() -> (ListBox, ButtonRow, ProgressBar) {
        let progress_bar = ProgressBar::builder()
            .text(t!("sidebar.progress_bar"))
            .show_text(true)
            .fraction(0.0)
            .build();
        progress_bar.set_hexpand(true);

        let progress_button = ButtonRow::builder()
            .child(&progress_bar)
            .hexpand(true)
            .build();

        let bottom_box = ListBox::builder()
            .css_classes(["navigation-sidebar"])
            .build();
        bottom_box.append(&progress_button);

        (bottom_box, progress_button, progress_bar)
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

    fn connect_action_manager_progess(&self, app: &Rc<App>) {
        let progress_bar_clone = self.progress_bar.clone();

        app.action_manager.listen(move |event: &TaskEvent| {
            dbg!("From progress bar!", event);

            match &event.event {
                TaskEventEnum::Progress {
                    task_name,
                    action,
                    action_nr,
                    total_actions,
                    progress,
                    status,
                } => {
                    progress_bar_clone.set_fraction(progress.clone());
                }
                _ => {}
            };
        });
    }

    fn connect_progress_button(&self) {
        let sidebar_clone = self.sidebar.clone();

        self.progress_button.connect_activated(move |_button_row| {
            sidebar_clone.set_selected(u32::MAX); // Unselect
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
