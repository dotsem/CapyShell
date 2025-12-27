//! Bluetooth monitoring service using BlueZ via D-Bus.
//!
//! Monitors bluetooth adapter state and connected devices.
//! Uses zbus for D-Bus signal monitoring with bluer for querying state.

use crate::panels::taskbar::events;
use log::{debug, error, info, warn};
use std::thread;

/// Bluetooth status for cross-thread communication.
#[derive(Clone, Debug)]
pub struct BluetoothStatus {
    /// Whether the adapter is powered on.
    pub powered: bool,
    /// Number of connected devices.
    pub connected_devices: u32,
    /// Whether discovery is active.
    pub discovering: bool,
    /// Adapter name (for future panel).
    pub adapter_name: Option<String>,
}

impl Default for BluetoothStatus {
    fn default() -> Self {
        Self {
            powered: false,
            connected_devices: 0,
            discovering: false,
            adapter_name: None,
        }
    }
}

/// Check if the system has any Bluetooth adapters available.
pub fn has_bluetooth() -> bool {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create tokio runtime for bluetooth check: {}", e);
            return false;
        }
    };

    rt.block_on(async { check_bluetooth_available().await })
}

/// Get current bluetooth status (blocking call for initial state).
pub fn get_status() -> BluetoothStatus {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create tokio runtime for bluetooth status: {}", e);
            return BluetoothStatus::default();
        }
    };

    rt.block_on(async { fetch_bluetooth_status().await })
}

/// Start the bluetooth monitoring background thread.
/// Returns true if bluetooth adapter was found and monitoring started.
pub fn start_monitor() -> bool {
    if !has_bluetooth() {
        info!("No Bluetooth adapter detected, skipping bluetooth monitor");
        return false;
    }

    info!("Starting bluetooth monitor...");

    thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                error!(
                    "Failed to create tokio runtime for bluetooth monitor: {}",
                    e
                );
                return;
            }
        };

        rt.block_on(async {
            if let Err(e) = dbus_worker().await {
                warn!("D-Bus worker failed: {}. Falling back to polling.", e);
                polling_worker().await;
            }
        });
    });

    true
}

// === Internal implementation ===

async fn check_bluetooth_available() -> bool {
    use bluer::Session;

    match Session::new().await {
        Ok(session) => match session.default_adapter().await {
            Ok(_) => true,
            Err(_) => match session.adapter_names().await {
                Ok(names) => !names.is_empty(),
                Err(_) => false,
            },
        },
        Err(e) => {
            debug!("Failed to create BlueZ session: {}", e);
            false
        }
    }
}

async fn fetch_bluetooth_status() -> BluetoothStatus {
    use bluer::Session;

    let session = match Session::new().await {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to connect to BlueZ: {}", e);
            return BluetoothStatus::default();
        }
    };

    let adapter = match session.default_adapter().await {
        Ok(a) => a,
        Err(e) => {
            debug!("Failed to get default adapter: {}", e);
            return BluetoothStatus::default();
        }
    };

    let powered = adapter.is_powered().await.unwrap_or(false);
    let discovering = adapter.is_discovering().await.unwrap_or(false);
    let adapter_name = adapter.alias().await.ok();
    let connected_devices = count_connected_devices(&adapter).await;

    BluetoothStatus {
        powered,
        connected_devices,
        discovering,
        adapter_name,
    }
}

async fn count_connected_devices(adapter: &bluer::Adapter) -> u32 {
    let device_addrs = match adapter.device_addresses().await {
        Ok(addrs) => addrs,
        Err(_) => return 0,
    };

    let mut count = 0;
    for addr in device_addrs {
        if let Ok(device) = adapter.device(addr) {
            if device.is_connected().await.unwrap_or(false) {
                count += 1;
            }
        }
    }
    count
}

/// Send bluetooth update to event bus (async version for use within dbus_worker).
async fn send_update() {
    let status = fetch_bluetooth_status().await;
    debug!(
        "Bluetooth update: powered={}, connected={}, discovering={}",
        status.powered, status.connected_devices, status.discovering
    );
    events::send_bluetooth(status);
}

async fn dbus_worker() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::stream::StreamExt;
    use zbus::Connection;

    let connection = Connection::system().await?;

    // Listen for BlueZ PropertiesChanged signals on all bluetooth objects
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .path_namespace("/org/bluez")?
        .build();

    let mut stream = zbus::MessageStream::for_match_rule(rule, &connection, Some(100)).await?;

    info!("Listening for BlueZ D-Bus signals...");

    // Send initial update
    send_update().await;

    while let Some(msg) = stream.next().await {
        if let Ok(_msg) = msg {
            // Any BlueZ property change triggers an update
            send_update().await;
        }
    }

    info!("D-Bus signal stream ended");
    Ok(())
}

async fn polling_worker() {
    use tokio::time::{Duration, sleep};

    info!("Using polling fallback (every 10 seconds)");
    loop {
        sleep(Duration::from_secs(10)).await;
        send_update().await;
    }
}
