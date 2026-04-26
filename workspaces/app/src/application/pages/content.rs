use crate::application::pages::page_config::TextAlign;

use super::{
    NavPage, NavPageBuild,
    page_config::{Content, Header, PageYaml},
};
use gtk::{
    Align, Image, Justification, Label, Orientation, ScrolledWindow,
    prelude::{BoxExt, WidgetExt},
};
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
    const SPACING: i32 = 20;
    const MAX_WIDTH: i32 = 600;

    pub fn from_yaml(page_yaml: &PageYaml) -> Rc<Self> {
        let title = &page_yaml.title;
        let icon = &page_yaml.icon;

        let NavPageBuild {
            nav_page,
            nav_row,
            toolbar,
            ..
        } = Self::build_nav_page(title, icon);

        let content_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .margin_top(Self::SPACING)
            .margin_bottom(Self::SPACING)
            .margin_start(Self::SPACING)
            .margin_end(Self::SPACING)
            .spacing(Self::SPACING)
            .build();
        let clamp = Clamp::builder()
            .maximum_size(Self::MAX_WIDTH)
            .child(&content_box)
            .build();
        let scrolled_window = ScrolledWindow::builder().child(&clamp).build();
        toolbar.set_content(Some(&scrolled_window));

        if let Some(header) = &page_yaml.header {
            let header_built = Self::build_header(header);
            content_box.append(&header_built);
        }

        if let Some(contents) = &page_yaml.contents {
            let content_built = Self::build_content(contents);
            content_box.append(&content_built);
        }

        Rc::new(Self { nav_page, nav_row })
    }

    fn build_header(header: &Header) -> gtk::Box {
        let content_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .halign(Align::Center)
            .valign(Align::Fill)
            .build();

        if let Some(icon_name) = &header.icon {
            let image = Image::builder()
                .icon_name(icon_name)
                .pixel_size(96)
                .margin_start(25)
                .margin_end(25)
                .css_classes(["icon-dropshadow"])
                .build();
            content_box.append(&image);
        }

        if let Some(text) = &header.text {
            let label = Label::builder()
                .label(text)
                .css_classes(["title-1"])
                .wrap(true)
                .build();
            content_box.append(&label);
        }

        content_box
    }

    fn build_content(contents: &Vec<Content>) -> gtk::Box {
        let content_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .build();

        for content in contents {
            let label = Label::builder()
                .use_markup(content.pango)
                .label(&content.text)
                .wrap(true)
                .halign(Align::Start)
                .justify(Justification::Left)
                .build();
            content_box.append(&label);

            match content.align {
                TextAlign::Left => {
                    label.set_halign(Align::Start);
                    label.set_justify(Justification::Left);
                }
                TextAlign::Center => {
                    label.set_halign(Align::Center);
                    label.set_justify(Justification::Center);
                }
                TextAlign::Fill => {
                    label.set_halign(Align::Fill);
                    label.set_justify(Justification::Fill);
                }
            }
        }

        content_box
    }
}
