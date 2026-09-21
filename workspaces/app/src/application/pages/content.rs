use super::{
    ContentNavPageBuild, DynPage, NavPage,
    content_yaml::{Content, ContentPageYaml, Header, TextAlign},
};
use crate::application::task_manager::{TaskManager, user_execution_context::UserExecutionContext};
use anyhow::Result;
use gtk::{
    Align, Image, Justification, Label, Orientation,
    prelude::{BoxExt, WidgetExt},
};
use libadwaita::NavigationPage;
use std::rc::Rc;

pub struct ContentPage {
    yaml: ContentPageYaml,
    nav_page: NavigationPage,
    content_box: gtk::Box,
}
impl DynPage for ContentPage {
    fn build_page(
        mut self,
        _task_manager: &Rc<TaskManager>,
        _user_context: &UserExecutionContext,
    ) -> Result<Rc<dyn DynPage>> {
        self.build();

        Ok(Rc::new(self))
    }
}
impl NavPage for ContentPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_section(&self) -> Option<&str> {
        self.yaml.section.as_deref()
    }

    fn get_icon(&self) -> Option<&str> {
        Some(&self.yaml.icon)
    }
}
impl ContentPage {
    pub fn new(yaml: ContentPageYaml) -> Self {
        let ContentNavPageBuild {
            nav_page,
            _toolbar,
            content,
        } = Self::build_content_nav_page(&yaml.title);

        Self {
            yaml,
            nav_page,
            content_box: content,
        }
    }

    fn build(&mut self) {
        if let Some(header) = &self.yaml.header {
            let header_built = Self::build_header(header);
            self.content_box.append(&header_built);
        }

        if let Some(contents) = &self.yaml.contents {
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
