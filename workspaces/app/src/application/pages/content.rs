use super::{ContentNavPageBuild, DynPage, NavPage, Page};
use crate::application::task_manager::TaskManager;
use gtk::{
    Align, Image, Justification, Label, Orientation,
    prelude::{BoxExt, WidgetExt},
};
use libadwaita::{NavigationPage, ToolbarView};
use serde::Deserialize;
use std::rc::Rc;

#[derive(Deserialize, PartialEq, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Fill,
}

#[derive(PartialEq, Deserialize, Debug)]
pub struct Header {
    pub icon: Option<String>,
    pub text: Option<String>,
}

#[derive(PartialEq, Deserialize, Debug)]
pub struct Content {
    #[serde(default)]
    pub pango: bool,

    #[serde(default)]
    pub align: TextAlign,

    pub text: String,
}

#[derive(PartialEq, Deserialize, Debug)]
pub struct ContentPage {
    title: String,
    section: Option<String>,
    icon: String,
    header: Option<Header>,
    contents: Option<Vec<Content>>,

    #[serde(skip)]
    nav_page: NavigationPage,
    #[serde(skip)]
    toolbar: ToolbarView,
    #[serde(skip)]
    content_box: gtk::Box,
}
impl DynPage for ContentPage {
    fn build_page(mut self, _task_manager: &Rc<TaskManager>) -> Page {
        let ContentNavPageBuild {
            nav_page,
            toolbar,
            content,
        } = Self::build_content_nav_page(&self.title);
        self.nav_page = nav_page;
        self.toolbar = toolbar;
        self.content_box = content;
        self.build();

        Rc::new(self)
    }
}
impl NavPage for ContentPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_section(&self) -> Option<&str> {
        self.section.as_deref()
    }

    fn get_icon(&self) -> Option<&str> {
        Some(&self.icon)
    }
}
impl ContentPage {
    fn build(&mut self) {
        if let Some(header) = &self.header {
            let header_built = Self::build_header(header);
            self.content_box.append(&header_built);
        }

        if let Some(contents) = &self.contents {
            let content_built = Self::build_content(contents);
            self.content_box.append(&content_built);
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
