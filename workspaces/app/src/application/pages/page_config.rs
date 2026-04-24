use super::{ContentPage, NavPage};
use anyhow::{Context, Result};
use gtk::{Align, Justification, Label, Orientation, prelude::BoxExt};
use libadwaita::{ActionRow, NavigationPage};
use serde::Deserialize;
use std::{fs, path::Path, rc::Rc};

#[derive(Debug, PartialEq, Deserialize)]
struct PageYaml {
    title: String,
    header_text: Option<String>,
}

pub struct Page {
    nav_page: NavigationPage,
    nav_row: ActionRow,
}
impl NavPage for Page {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_nav_row(&self) -> Option<&ActionRow> {
        Some(&self.nav_row)
    }
}
impl Page {
    pub fn new(file_path: &Path) -> Result<Rc<Self>> {
        let file_string = fs::read_to_string(file_path).context(format!(
            "Failed to read file to string: {}",
            file_path.display()
        ))?;

        let page_yaml: PageYaml = serde_yaml::from_str(&file_string)
            .context(format!("Failed to parse yml: {}", file_path.display()))?;

        let title = page_yaml.title;
        let icon = "go-home-symbolic";

        let ContentPage {
            nav_page,
            nav_row,
            content_box,
            ..
        } = Self::build_nav_page(&title, icon).with_content_box();

        if let Some(header_text) = &page_yaml.header_text {
            let header_text = Self::build_header_text(header_text);
            content_box.append(&header_text);
        }

        Ok(Rc::new(Self { nav_page, nav_row }))
    }

    pub fn get_fallback() -> Rc<Self> {
        let icon = "go-home-symbolic";
        let title = t!("pages.fallback.title");

        let ContentPage {
            nav_page,
            nav_row,
            content_box,
            ..
        } = Self::build_nav_page(&title, icon).with_content_box();

        let header_text = Self::build_header_text(&t!("pages.fallback.get_started").to_string());
        content_box.append(&header_text);

        Rc::new(Self { nav_page, nav_row })
    }

    fn build_header_text(text: &String) -> gtk::Box {
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
