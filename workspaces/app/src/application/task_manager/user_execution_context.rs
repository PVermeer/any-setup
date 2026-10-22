use anyhow::Result;
use common::{
    app_dirs::AppDirs,
    cli::Cli,
    dbus_query::{self, DbusConnectionType},
    utils,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap, os::unix::process::CommandExt, path::PathBuf, process::Command, rc::Rc,
    sync::Arc,
};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct UserExecutionContext {
    pub user_id: u32,
    pub user_name: String,
    pub current_dir: PathBuf,
    pub system_config_dir: PathBuf,
    pub system_data_dir: PathBuf,
    pub pages_dir: Option<PathBuf>,
    pub configs_dir: Option<PathBuf>,
    pub environment: HashMap<String, String>,
    pub arguments: Cli,
}

impl UserExecutionContext {
    pub fn new(arguments: Cli, app_dirs: &Rc<AppDirs>) -> Result<Arc<Self>> {
        let environment = HashMap::from([
            (
                "XDG_RUNTIME_DIR".to_string(),
                utils::env::get_user_runtime_dir(),
            ),
            (
                "DBUS_SESSION_BUS_ADDRESS".to_string(),
                dbus_query::get_address(&DbusConnectionType::Session)?,
            ),
        ]);

        Ok(Arc::new(Self {
            user_id: utils::env::get_user_id(),
            user_name: utils::env::get_user_name(),
            current_dir: utils::env::get_current_dir(),
            system_config_dir: app_dirs.system_config.clone(),
            system_data_dir: app_dirs.system_data.clone(),
            pages_dir: app_dirs.app_system_data_pages(),
            configs_dir: app_dirs.app_system_data_configs(),
            environment,
            arguments,
        }))
    }

    pub fn apply_to(&self, command: &mut Command) {
        command.envs(&self.environment);
        command.uid(self.user_id);
        command.current_dir(&self.current_dir);
    }
}
