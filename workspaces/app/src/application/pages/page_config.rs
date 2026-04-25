use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::PathBuf};

#[derive(Deserialize, PartialEq)]
pub enum PageType {
    #[serde(alias = "content", alias = "CONTENT")]
    Content,
}

#[derive(PartialEq, Deserialize)]
pub struct PageYaml {
    pub page_type: PageType,
    pub title: String,
    pub icon: String,
    pub header_text: Option<String>,
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
