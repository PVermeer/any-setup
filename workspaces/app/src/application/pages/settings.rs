use std::rc::Rc;

use crate::application::pages::{DynPage, NavPage, NavPageBuild, Page};
use gtk::{
    Align, Image, Justification, Label, Orientation, ScrolledWindow,
    prelude::{BoxExt, WidgetExt},
};
use libadwaita::{ActionRow, Clamp, NavigationPage, ToolbarView};
use serde::Deserialize;

#[derive(Deserialize, PartialEq, Default)]
pub enum TextAlign {
    #[default]
    #[serde(alias = "left", alias = "LEFT")]
    Left,

    #[serde(alias = "center", alias = "CENTER")]
    Center,

    #[serde(alias = "fill", alias = "FILL")]
    Fill,
}

#[derive(PartialEq, Deserialize)]
pub struct Header {
    pub icon: Option<String>,
    pub text: Option<String>,
}

#[derive(PartialEq, Deserialize)]
pub struct Content {
    #[serde(default)]
    pub pango: bool,

    #[serde(default)]
    pub align: TextAlign,

    pub text: String,
}

#[derive(PartialEq, Deserialize)]
pub struct SettingsPage {
    title: String,
    icon: String,
    header: Option<Header>,
    contents: Option<Vec<Content>>,

    #[serde(skip)]
    nav_page: NavigationPage,
    #[serde(skip)]
    nav_row: ActionRow,
    #[serde(skip)]
    toolbar: ToolbarView,
}
impl DynPage for SettingsPage {
    fn build_page(mut self) -> Page {
        let NavPageBuild {
            nav_page,
            nav_row,
            toolbar,
        } = Self::build_nav_page(&self.title, &self.icon);
        self.nav_page = nav_page;
        self.nav_row = nav_row;
        self.toolbar = toolbar;
        self.build();

        Rc::new(self)
    }
}
impl NavPage for SettingsPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_nav_row(&self) -> &ActionRow {
        &self.nav_row
    }
}
impl SettingsPage {
    const SPACING: i32 = 20;
    const MAX_WIDTH: i32 = 600;

    pub fn build(&mut self) {
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
        self.toolbar.set_content(Some(&scrolled_window));

        if let Some(header) = &self.header {
            let header_built = Self::build_header(header);
            content_box.append(&header_built);
        }

        if let Some(contents) = &self.contents {
            let content_built = Self::build_content(contents);
            content_box.append(&content_built);
        }
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
