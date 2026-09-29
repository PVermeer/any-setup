use crate::{
    config::{self},
    utils::OnceLockExt,
};
use anyhow::{Context, Result};
use gtk::glib;
use std::{fs, path::PathBuf, rc::Rc};
use tracing::{debug, error, info, warn};

#[derive(Default, Debug)]
pub struct AppDirs {
    pub user_data: PathBuf,
    pub user_home: PathBuf,
    pub user_config: PathBuf,
    pub user_cache: PathBuf,
    pub user_runtime: PathBuf,
    pub system_data: PathBuf,
    system_config: PathBuf,
    pages_dir_run_argument: Option<PathBuf>,
}
impl AppDirs {
    const PAGES_DIR_NAME: &str = "pages";

    pub fn new(pages_dir: &Option<PathBuf>) -> Result<Rc<Self>> {
        let user_home = glib::home_dir();
        let user_data = glib::user_data_dir();
        let user_config = glib::user_config_dir();
        let user_cache = glib::user_cache_dir();
        let user_runtime = glib::user_runtime_dir();

        // Ordered list by XDG_DATA_DIRS spec
        let mut system_data_dirs = glib::system_data_dirs();
        // Reverse the order so the lower level data, set by a packager or admin, always take precedence
        system_data_dirs.reverse();

        debug!(?system_data_dirs, "System data dirs");

        let system_data = system_data_dirs
            .first()
            .cloned()
            .context("Failed to get system data dir")?;

        // For some reason..., XDG_CONFIG_DIRS == /etc/xdg
        let system_config = PathBuf::from("/etc");

        let me = Self {
            pages_dir_run_argument: pages_dir.clone(),
            user_home,
            user_data,
            user_config,
            user_cache,
            user_runtime,
            system_data,
            system_config,
        };

        debug!(?me, "Application dirs");

        Ok(Rc::new(me))
    }

    pub fn app_user_data(&self) -> Result<PathBuf> {
        let path = self.user_data.join(config::APP_NAME_HYPHEN.get_value());

        if !path.is_dir() {
            fs::create_dir_all(&path)
                .context(format!("Failed to create app_data dir: {}", path.display()))?;
        }

        Ok(path)
    }

    pub fn app_user_config(&self) -> Result<PathBuf> {
        let path = self.user_config.join(config::APP_NAME_HYPHEN.get_value());

        if !path.is_dir() {
            fs::create_dir_all(&path).context(format!(
                "Failed to create app_config dir: {}",
                path.display()
            ))?;
        }

        Ok(path)
    }

    pub fn app_system_data(&self) -> Option<PathBuf> {
        let path = self
            .system_data
            .clone()
            .join(config::APP_NAME_HYPHEN.get_value());

        if !path.is_dir() {
            warn!(path = %path.display(), "App system data path does not exists");
            return None;
        }

        Some(path)
    }

    pub fn app_system_data_pages(&self) -> Option<PathBuf> {
        let mut path = if let Some(pages_dir) = &self.pages_dir_run_argument {
            info!(?pages_dir, "Found user set pages dir argument");

            let pages_dir_abs = pages_dir.canonicalize().unwrap_or_default();

            if !pages_dir_abs.is_dir() {
                error!(?pages_dir_abs, "User set pages dir is not a directory");
            }

            Some(pages_dir_abs)
        } else {
            self.app_system_data()
                .map(|dir| dir.join(Self::PAGES_DIR_NAME))
                .filter(|dir| {
                    let exists = dir.is_dir();
                    if !exists {
                        warn!(path = %dir.display(), "App system pages path does not exists");
                    }

                    exists
                })
        };

        if cfg!(debug_assertions) && self.pages_dir_run_argument.is_none() {
            path = Some(
                glib::current_dir()
                    .join("dev-assets")
                    .join(Self::PAGES_DIR_NAME),
            );
        }

        path
    }

    pub fn app_system_config(&self) -> Option<PathBuf> {
        let path = self
            .system_config
            .clone()
            .join(config::APP_NAME_HYPHEN.get_value());

        if !path.is_dir() {
            warn!(path = %path.display(), "App system config path does not exists");
            return None;
        }

        Some(path)
    }

    /// Cache dir is always user based
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
