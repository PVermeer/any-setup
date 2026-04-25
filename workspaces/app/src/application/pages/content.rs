use super::{NavPage, NavPageBuild, PageYaml};
use gtk::{Align, Justification, Label, Orientation, ScrolledWindow, prelude::BoxExt};
use libadwaita::{ActionRow, Clamp, NavigationPage};
use std::rc::Rc;

pub struct ContentPage {
    nav_page: NavigationPage,
    nav_row: ActionRow,
}
impl NavPage for ContentPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_nav_row(&self) -> &ActionRow {
        &self.nav_row
    }
}
impl ContentPage {
    pub fn from_yaml(page_yaml: &PageYaml) -> Rc<Self> {
        let title = &page_yaml.title;
        let icon = &page_yaml.icon;

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

        if let Some(header_text) = &page_yaml.header_text {
            let header_text = Self::build_header_text(header_text);
            content_box.append(&header_text);
        }

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
