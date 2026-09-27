use crate::{
    config::{self},
    utils::OnceLockExt,
};
use anyhow::{Context, Result};
use gtk::glib;
use std::{fs, path::PathBuf, rc::Rc};
use tracing::{debug, error, info};

#[derive(Default, Debug)]
pub struct AppDirs {
    pub user_home: PathBuf,
    pub user_data: PathBuf,
    pub user_config: PathBuf,
    pub user_cache: PathBuf,
    pub user_runtime: PathBuf,
    pub system_data_dir: Option<PathBuf>,
    pub system_data_pages_dir: Option<PathBuf>,
}
impl AppDirs {
    pub fn new(pages_dir: &Option<PathBuf>) -> Result<Rc<Self>> {
        let user_home = glib::home_dir();
        let user_data = glib::user_data_dir();
        let user_config = glib::user_config_dir();
        let user_cache = glib::user_cache_dir();
        let user_runtime = glib::user_runtime_dir();

        let system_data_dirs = glib::system_data_dirs();
        debug!(?system_data_dirs, "System data dirs");

        let mut system_data_dir = system_data_dirs
            .into_iter()
            .find(|dir| dir.join(config::APP_NAME_HYPHEN.get_value()).is_dir());

        if cfg!(debug_assertions) {
            system_data_dir = Some(glib::current_dir().join("dev-assets").join("share"));
        }
        let mut system_data_pages_dir = system_data_dir.clone().map(|dir| dir.join("pages"));

        if let Some(pages_dir) = pages_dir {
            info!(?pages_dir, "Found user set pages dir argument");

            let pages_dir_abs = pages_dir.canonicalize().unwrap_or_default();

            if !pages_dir_abs.is_dir() {
                error!(?pages_dir_abs, "User set pages dir is not a directory");
            }
            system_data_pages_dir = Some(pages_dir_abs);
        }

        let me = Self {
            user_home,
            user_data,
            user_config,
            user_cache,
            user_runtime,
            system_data_dir,
            system_data_pages_dir,
        };

        debug!(?me, "Application dirs");

        Ok(Rc::new(me))
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
