use anyhow::Result;
use common::{
    dbus_query::{self, DbusConnectionType},
    utils,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap, os::unix::process::CommandExt, path::PathBuf, process::Command, sync::Arc,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserExecutionContext {
    pub user_id: u32,
    pub user_name: String,
    pub current_dir: PathBuf,
    pub environment: HashMap<String, String>,
}
impl UserExecutionContext {
    pub fn new() -> Result<Arc<Self>> {
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
            environment,
        }))
    }

    pub fn apply_to(&self, command: &mut Command) {
        command.envs(&self.environment);
        command.uid(self.user_id);
        command.current_dir(&self.current_dir);
    }
}
