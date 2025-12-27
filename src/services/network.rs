//! Network monitoring service using NetworkManager via D-Bus.
//!
//! Monitors network state and broadcasts changes to all panels via the event bus.
//! Uses zbus for D-Bus signal monitoring with nmrs for querying state.

use crate::panels::taskbar::events;
use log::{debug, error, info, warn};
use std::thread;

/// Connection type for the current network.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ConnectionType {
    #[default]
    None,
    Wifi,
    Ethernet,
}

/// Network status for cross-thread communication.
#[derive(Clone, Debug)]
pub struct NetworkStatus {
    /// Whether connected to any network.
    pub connected: bool,
    /// WiFi signal strength (0-100), None if not WiFi or unknown.
    pub signal_strength: Option<u8>,
    /// Connected SSID for WiFi (for future network panel).
    pub ssid: Option<String>,
    /// Type of connection.
    pub connection_type: ConnectionType,
}

impl Default for NetworkStatus {
    fn default() -> Self {
        Self {
            connected: false,
            signal_strength: None,
            ssid: None,
            connection_type: ConnectionType::None,
        }
    }
}

/// Get current network status (blocking call for initial state).
pub fn get_status() -> NetworkStatus {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create tokio runtime for network status: {}", e);
            return NetworkStatus::default();
        }
    };

    rt.block_on(async { fetch_network_status().await })
}

/// Start the network monitoring background thread.
pub fn start_monitor() {
    info!("Starting network monitor...");

    thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                error!("Failed to create tokio runtime for network monitor: {}", e);
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
}

// === Internal implementation ===

async fn fetch_network_status() -> NetworkStatus {
    use nmrs::NetworkManager;

    let nm = match NetworkManager::new().await {
        Ok(nm) => nm,
        Err(e) => {
            warn!("Failed to connect to NetworkManager: {}", e);
            return NetworkStatus::default();
        }
    };

    // Check current connection
    let ssid = nm.current_ssid().await;
    let connected = ssid.is_some();

    if !connected {
        return check_ethernet_status(&nm).await;
    }

    // We have WiFi - get signal strength
    let signal_strength = get_current_signal_strength(&nm, ssid.as_deref()).await;

    NetworkStatus {
        connected: true,
        signal_strength,
        ssid,
        connection_type: ConnectionType::Wifi,
    }
}

async fn check_ethernet_status(nm: &nmrs::NetworkManager) -> NetworkStatus {
    match nm.list_devices().await {
        Ok(devices) => {
            for device in devices {
                if device.device_type == nmrs::DeviceType::Ethernet {
                    if device.state == nmrs::DeviceState::Activated {
                        return NetworkStatus {
                            connected: true,
                            signal_strength: None,
                            ssid: None,
                            connection_type: ConnectionType::Ethernet,
                        };
                    }
                }
            }
        }
        Err(e) => {
            debug!("Failed to list network devices: {}", e);
        }
    }

    NetworkStatus::default()
}

async fn get_current_signal_strength(
    nm: &nmrs::NetworkManager,
    current_ssid: Option<&str>,
) -> Option<u8> {
    let Some(ssid) = current_ssid else {
        return None;
    };

    match nm.list_networks().await {
        Ok(networks) => {
            for net in networks {
                if net.ssid == ssid {
                    return net.strength;
                }
            }
        }
        Err(e) => {
            debug!("Failed to list networks for signal strength: {}", e);
        }
    }

    None
}

/// Send network update to event bus (async version for use within dbus_worker).
async fn send_update() {
    let status = fetch_network_status().await;
    debug!(
        "Network update: connected={}, type={:?}, signal={:?}",
        status.connected, status.connection_type, status.signal_strength
    );
    events::send_network(status);
}

async fn dbus_worker() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::stream::StreamExt;
    use zbus::Connection;

    let connection = Connection::system().await?;

    // Listen for NetworkManager StateChanged and PropertiesChanged signals
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .path_namespace("/org/freedesktop/NetworkManager")?
        .build();

    let mut stream = zbus::MessageStream::for_match_rule(rule, &connection, Some(100)).await?;

    info!("Listening for NetworkManager D-Bus signals...");

    // Send initial update
    send_update().await;

    while let Some(msg) = stream.next().await {
        if let Ok(_msg) = msg {
            // Any NetworkManager property change triggers an update
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
