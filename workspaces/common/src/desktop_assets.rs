use crate::{assets, config, utils::OnceLockExt};
use anyhow::Result;
use freedesktop_desktop_entry::DesktopEntry;
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use tracing::{error, info};

pub fn create_app_desktop_file() -> Result<PathBuf> {
    info!("==== Creating app desktop file");

    let desktop_file = assets::get_desktop_file_in();
    let app_id = config::APP_ID.get_value();
    let app_name = config::APP_NAME.get_value();
    let app_summary = config::APP_SUMMARY.get_value();
    let bin_name = config::BIN_NAME.get_value();
    let file_name = desktop_file_name();
    let save_path = assets_desktop_path().join(file_name);

    let mut base_desktop_file =
        DesktopEntry::from_str(&save_path, desktop_file, None::<&[String]>)?;

    base_desktop_file.add_desktop_entry("Name".to_string(), app_name.clone());
    base_desktop_file.add_desktop_entry("Icon".to_string(), app_id.clone());
    base_desktop_file.add_desktop_entry("StartupWMClass".to_string(), app_id.clone());
    base_desktop_file.add_desktop_entry("Exec".to_string(), bin_name.clone());
    base_desktop_file.add_desktop_entry("Comment".to_string(), app_summary.clone());

    fs::write(&save_path, base_desktop_file.to_string()).inspect_err(|err| {
        error!(
            error = err.to_string(),
            path = &save_path.to_string_lossy().to_string(),
            "Failed to save desktop file"
        );
    })?;

    info!(
        desktop_file = &save_path.to_string_lossy().to_string(),
        "Created desktop file:"
    );

    Ok(save_path)
}

pub fn create_app_icon() -> Result<PathBuf> {
    info!("==== Creating app icon");

    let file_name = icon_file_name();
    let save_path = assets_desktop_path().join(file_name);

    let mut icon_file = File::create(&save_path)?;
    icon_file
        .write_all(assets::get_icon_data_in())
        .inspect_err(|err| {
            error!(
                error = err.to_string(),
                path = &save_path.to_string_lossy().to_string(),
                "Failed to save flatpak manifest"
            );
        })?;

    info!(
        app_icon = &save_path.to_string_lossy().to_string(),
        "Created app icon:"
    );

    Ok(save_path)
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

fn project_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("Project path not found")
}

fn assets_path() -> PathBuf {
    let path = project_path().join("assets");
    if !path.is_dir() {
        fs::create_dir_all(&path).expect("Failed to create assets path");
    }
    path
}

fn assets_desktop_path() -> PathBuf {
    let path = assets_path().join("desktop");
    if !path.is_dir() {
        fs::create_dir_all(&path).expect("Failed to create desktop assets path");
    }
    path
}
