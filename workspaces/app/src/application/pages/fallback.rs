use crate::application::{
    action_manager::ActionManager,
    pages::{DynPage, NavPage, NavPageBuild, Page},
};
use gtk::{Align, Justification, Label, Orientation, ScrolledWindow, prelude::BoxExt};
use libadwaita::{Clamp, NavigationPage};
use std::rc::Rc;

pub struct FallbackPage {
    nav_page: NavigationPage,
    icon: String,
}
impl DynPage for FallbackPage {
    fn build_page(self, _action_manager: &Rc<ActionManager>) -> Page {
        Rc::new(self)
    }
}
impl NavPage for FallbackPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_section(&self) -> Option<&str> {
        None
    }

    fn get_icon(&self) -> Option<&str> {
        Some(&self.icon)
    }
}
impl FallbackPage {
    pub fn new() -> Self {
        let icon = "go-home-symbolic".to_string();
        let title = &t!("pages.fallback.title");

        let NavPageBuild {
            nav_page, toolbar, ..
        } = Self::build_nav_page(title);

        let margin = 20;
        let max_width = 600;

        let content_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .margin_top(margin)
            .margin_bottom(margin)
            .margin_start(margin)
            .margin_end(margin)
            .build();
        let clamp = Clamp::builder()
            .maximum_size(max_width)
            .child(&content_box)
            .build();
        let scrolled_window = ScrolledWindow::builder().child(&clamp).build();
        toolbar.set_content(Some(&scrolled_window));

        let header_text = Self::build_header_text(&t!("pages.fallback.get_started"));
        content_box.append(&header_text);

        Self { nav_page, icon }
    }

    fn build_header_text(text: &str) -> gtk::Box {
        let content_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .halign(Align::Center)
            .spacing(12)
            .build();

        let text = Label::builder()
            .label(text)
            .css_classes(["label-spaced"])
            .wrap(true)
            .justify(Justification::Center)
            .build();

        content_box.append(&text);

        content_box
    }
}
