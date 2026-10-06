use anyhow::Context;
use common::{app_dirs::AppDirs, config, utils::OnceLockExt};
use serde::Deserialize;
use std::{fs, rc::Rc, sync::OnceLock};
use tracing::error;

pub static APP_NAME: OnceLock<String> = OnceLock::new();
pub static APP_ICON: OnceLock<String> = OnceLock::new();

#[derive(Deserialize)]
struct AppConfig {
    app_name: String,
    app_icon: String,
}

pub fn init(app_dirs: &Rc<AppDirs>) {
    let app_config = app_dirs
        .app_system_data()
        .map(|dir| dir.join("config.yml"))
        .filter(|file_path| file_path.exists())
        .and_then(|file_path| {
            fs::read_to_string(&file_path)
                .context("Failed to read file to string")
                .and_then(|file_string| {
                    serde_yaml::from_str::<AppConfig>(&file_string)
                        .context("Could not parse the app config file")
                })
                .inspect_err(|error| error!(%error, ?file_path))
                .ok()
        });

    if let Some(app_config) = app_config {
        let _ = APP_NAME.set(app_config.app_name);
        let _ = APP_ICON.set(app_config.app_icon);
    } else {
        let _ = APP_NAME.set(config::APP_NAME.get_value().clone());
        let _ = APP_ICON.set(config::APP_ID.get_value().clone());
    }
}
