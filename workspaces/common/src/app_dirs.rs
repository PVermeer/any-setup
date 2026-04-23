use crate::{
    config::{self},
    utils::OnceLockExt,
};
use anyhow::{Context, Result};
use gtk::glib;
use std::{fs, path::PathBuf, rc::Rc};

#[derive(Default)]
pub struct AppDirs {
    pub user_home: PathBuf,
    pub user_data: PathBuf,
    pub user_config: PathBuf,
    pub user_cache: PathBuf,
}
impl AppDirs {
    pub fn new() -> Result<Rc<Self>> {
        Rc::new(Self::default());

        let user_home = glib::home_dir();
        let user_data = glib::user_data_dir();
        let user_config = glib::user_config_dir();
        let user_cache = glib::user_cache_dir();

        Ok(Rc::new(Self {
            user_home,
            user_data,
            user_config,
            user_cache,
        }))
    }

    pub fn app_data(&self) -> Result<PathBuf> {
        let path = self.user_data.join(config::APP_NAME_HYPHEN.get_value());

        if !path.is_dir() {
            fs::create_dir_all(&path)
                .context(format!("Failed to create app_data dir: {}", path.display()))?;
        }

        Ok(path)
    }

    pub fn app_config(&self) -> Result<PathBuf> {
        let path = self.user_config.join(config::APP_NAME_HYPHEN.get_value());

        if !path.is_dir() {
            fs::create_dir_all(&path).context(format!(
                "Failed to create app_config dir: {}",
                path.display()
            ))?;
        }

        Ok(path)
    }

    pub fn app_cache(&self) -> Result<PathBuf> {
        let path = self.user_cache.join(config::APP_NAME_HYPHEN.get_value());

        if !path.is_dir() {
            fs::create_dir_all(&path).context(format!(
                "Failed to create app_cache dir: {}",
                path.display()
            ))?;
        }

        Ok(path)
    }
}
