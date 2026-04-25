use super::{NavPage, NavPageBuild};
use gtk::{Align, Justification, Label, Orientation, ScrolledWindow, prelude::BoxExt};
use libadwaita::{ActionRow, Clamp, NavigationPage};
use std::rc::Rc;

pub struct FallbackPage {
    nav_page: NavigationPage,
    nav_row: ActionRow,
}
impl NavPage for FallbackPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_nav_row(&self) -> &ActionRow {
        &self.nav_row
    }
}
impl FallbackPage {
    pub fn new() -> Rc<Self> {
        let icon = "go-home-symbolic";
        let title = &t!("pages.fallback.title");

        let NavPageBuild {
            nav_page,
            nav_row,
            toolbar,
            ..
        } = Self::build_nav_page(title, icon);

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

        Rc::new(Self { nav_page, nav_row })
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
