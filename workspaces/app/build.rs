#![allow(dead_code)]

use anyhow::{Context, Result};
use common::{
    app_dirs::AppDirs,
    config::{self},
    desktop_assets,
    utils::{self, OnceLockExt},
};
use std::{
    fs::{self},
    path::{Path, PathBuf},
};

fn main() -> Result<()> {
    println!("cargo:warning=Debug: App build script is running!");
    config::init();
    let app_dirs = AppDirs::new()?;

    create_config_symlinks(&app_dirs);
    create_data_symlinks(&app_dirs);
    create_cache_symlinks(&app_dirs);

    install_app_desktop_file(&app_dirs)?;
    install_app_icon(&app_dirs)?;

    Ok(())
}

fn create_config_symlinks(app_dirs: &AppDirs) {
    let config_path = dev_config_path();
    let Ok(app_config_path) = app_dirs.app_config() else {
        return;
    };

    let _ = utils::files::create_symlink(&config_path, &app_config_path);
}

fn create_data_symlinks(app_dirs: &AppDirs) {
    let data_path = dev_data_path();
    let Ok(app_data_path) = app_dirs.app_data() else {
        return;
    };

    let _ = utils::files::create_symlink(&data_path, &app_data_path);
}

fn create_cache_symlinks(app_dirs: &AppDirs) {
    let cache_path = dev_cache_path();
    let Ok(app_cache_path) = app_dirs.app_cache() else {
        return;
    };

    let _ = utils::files::create_symlink(&cache_path, &app_cache_path);
}

fn install_app_desktop_file(app_dirs: &AppDirs) -> Result<()> {
    let desktop_file = desktop_assets::create_app_desktop_file()?;
    let file_name = desktop_file
        .file_name()
        .context("No file name on app-desktop-file")?;
    let save_dir = app_dirs.user_data.join("applications");
    if !save_dir.is_dir() {
        fs::create_dir_all(&save_dir).context("Failed to create applications dir")?;
    }
    let save_file = save_dir.join(file_name);

    fs::copy(desktop_file, save_file).context("Desktop file copy failed")?;
    Ok(())
}

fn install_app_icon(app_dirs: &AppDirs) -> Result<()> {
    let icon_file = desktop_assets::create_app_icon()?;
    let file_name = icon_file
        .file_name()
        .context("No file name on app-icon-file")?;

    let save_dir = app_dirs
        .user_data
        .join("icons")
        .join("hicolor")
        .join("256x256")
        .join("apps");
    if !save_dir.is_dir() {
        fs::create_dir_all(&save_dir).context("Failed to create icon dir")?;
    }

    let save_file = save_dir.join(file_name);

    fs::copy(icon_file, save_file).context("Icon copy failed")?;
    Ok(())
}

fn project_path() -> PathBuf {
    Path::new("").join("..").join("..").canonicalize().unwrap()
}

fn assets_path() -> PathBuf {
    project_path().join("assets")
}

fn dev_config_path() -> PathBuf {
    project_path().join("dev-config")
}

fn dev_data_path() -> PathBuf {
    project_path().join("dev-data")
}

fn dev_cache_path() -> PathBuf {
    project_path().join("dev-cache")
}

fn dev_assets_path() -> PathBuf {
    project_path().join("dev-assets")
}

fn desktop_file_name() -> String {
    let app_id = config::APP_ID.get_value();
    let extension = "desktop";
    let file_name = format!("{app_id}.{extension}");

    file_name
}

fn icon_file_name() -> String {
    let app_id = config::APP_ID.get_value();
    let extension = "png";
    let file_name = format!("{app_id}.{extension}");

    file_name
}
