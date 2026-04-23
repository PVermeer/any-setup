use std::{fs, path::PathBuf};

use crate::app_dirs::AppDirs;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, instrument};

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct CacheWindowSettings {
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub maximized: bool,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct CacheSettingsYaml {
    #[serde(default)]
    pub window: CacheWindowSettings,
}

#[derive(Debug)]
pub struct CacheSettings {
    pub settings: CacheSettingsYaml,
    settings_path: PathBuf,
}
impl CacheSettings {
    const CACHE_SETTINGS_FILE: &str = "settings.yml";

    pub fn new(app_dirs: &AppDirs) -> Result<Self> {
        let settings_path = app_dirs.app_cache()?.join(Self::CACHE_SETTINGS_FILE);
        let yaml_string = fs::read_to_string(&settings_path).unwrap_or_default();

        let settings: CacheSettingsYaml = serde_yaml::from_str(&yaml_string)
            .inspect_err(
                |error| error!(%error, path = %settings_path.display(), "Failed to parse cached settings yaml file"),
            )
            .unwrap_or_default();

        Ok(Self {
            settings,
            settings_path,
        })
    }

    pub fn set_window_size(&mut self, width: i32, height: i32, maximized: bool) {
        self.settings.window.width = width;
        self.settings.window.height = height;
        self.settings.window.maximized = maximized;
    }

    pub fn reset(&mut self) {
        self.settings = CacheSettingsYaml::default();
        let _ = self.save();
    }

    #[instrument(err, skip(self))]
    pub fn save(&self) -> Result<()> {
        debug!("Saving settings in cache");

        let yaml_string = serde_yaml::to_string(&self.settings)
            .context("Failed to parse settings to yaml string")?;

        fs::write(&self.settings_path, &yaml_string).context(format!(
            "Failed to write new settings file: {}",
            self.settings_path.display()
        ))?;

        Ok(())
    }
}
