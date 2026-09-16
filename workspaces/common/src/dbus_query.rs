use anyhow::{Context, Result};
pub use dbus::{
    self,
    blocking::{Connection, stdintf::org_freedesktop_dbus::Properties},
};
use std::time::Duration;

#[derive(Debug)]
pub enum DbusConnectionType {
    System,
    Session,
}

#[derive(Debug)]
pub struct DbusPropertyQuery<'a> {
    pub connection_type: DbusConnectionType,
    pub destination: &'a str,
    pub path: &'a str,
    pub interface: &'a str,
    pub property: &'a str,
}

pub fn get_address(connection_type: &DbusConnectionType) -> Result<String> {
    match connection_type {
        DbusConnectionType::System => std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .context("DBUS_SESSION_BUS_ADDRESS environment variable is not defined"),

        DbusConnectionType::Session => std::env::var("DBUS_SYSTEM_BUS_ADDRESS")
            .or(Ok("unix:path=/var/run/dbus/system_bus_socket".into())),
    }
}

#[tracing::instrument(err)]
pub fn get_property<T>(query: DbusPropertyQuery) -> Result<T>
where
    T: for<'a> dbus::arg::Get<'a> + 'static,
{
    let DbusPropertyQuery {
        connection_type,
        destination,
        path,
        interface,
        property,
    } = query;

    let connection = match connection_type {
        DbusConnectionType::System => Connection::new_system(),
        DbusConnectionType::Session => Connection::new_session(),
    }
    .context("Failed to create dbus connection")?;

    let proxy = connection.with_proxy(destination, path, Duration::from_secs(5));

    let property: T = proxy
        .get(interface, property)
        .context("Failed to get dbus property")?;

    Ok(property)
}
