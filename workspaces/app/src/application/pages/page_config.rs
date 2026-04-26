use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::PathBuf};

#[derive(Deserialize, PartialEq)]
pub enum PageType {
    #[serde(alias = "content", alias = "CONTENT")]
    Content,
}

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
pub struct PageYaml {
    pub page_type: PageType,
    pub title: String,
    pub icon: String,
    pub header: Option<Header>,
    pub contents: Option<Vec<Content>>,
}
impl PageYaml {
    pub fn from_file(file_path: &PathBuf) -> Result<Self> {
        let file_string = fs::read_to_string(file_path).context(format!(
            "Failed to read file to string: {}",
            file_path.display()
        ))?;

        serde_yaml::from_str(&file_string)
            .context(format!("Not a valid page yaml: {}", file_path.display()))
    }
}
