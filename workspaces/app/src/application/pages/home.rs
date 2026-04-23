use crate::application::{
    App,
    pages::{ContentPage, NavPage},
};
use common::{
    config::{self},
    utils::OnceLockExt,
};
use gtk::{Align, Orientation, prelude::WidgetExt};
use libadwaita::{
    ActionRow, NavigationPage,
    gtk::{self, Label, prelude::BoxExt},
};
use std::rc::Rc;

pub struct HomePage {
    nav_page: NavigationPage,
    nav_row: ActionRow,
    content_box: gtk::Box,
}
impl NavPage for HomePage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_nav_row(&self) -> Option<&ActionRow> {
        Some(&self.nav_row)
    }
}
impl HomePage {
    pub fn new() -> Rc<Self> {
        let title = t!("home.title");
        let icon = "go-home-symbolic";

        let ContentPage {
            nav_page,
            nav_row,
            content_box,
            ..
        } = Self::build_nav_page(&title, icon).with_content_box();

        Rc::new(Self {
            nav_page,
            nav_row,
            content_box,
        })
    }

    pub fn init(&self, app: &Rc<App>) {
        self.content_box.set_spacing(24);

        let header = Self::build_header(app);

        self.content_box.append(&header);
    }

    fn build_header(app: &Rc<App>) -> gtk::Box {
        let content_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .halign(Align::Center)
            .valign(Align::Fill)
            .build();

        let icon = app.get_icon();
        icon.set_pixel_size(96);
        icon.set_css_classes(&["icon-dropshadow"]);
        icon.set_margin_start(25);
        icon.set_margin_end(25);

        let name = Label::builder()
            .label(config::APP_NAME.get_value())
            .css_classes(["title-1"])
            .wrap(true)
            .build();

        content_box.append(&icon);
        content_box.append(&name);

        content_box
    }
}
