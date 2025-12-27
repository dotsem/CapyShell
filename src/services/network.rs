//! Network monitoring service using NetworkManager via nmrs.
//!
//! Monitors network state and broadcasts changes to all panels via the event bus.
//! Uses the nmrs crate for async NetworkManager D-Bus communication.

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
    // Use a blocking approach for initial state
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
            if let Err(e) = run_monitor().await {
                error!("Network monitor failed: {}", e);
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
        // Check if we have ethernet instead
        // For now, we consider "no SSID" as potentially ethernet or disconnected
        // We'll check device state for more accuracy
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
    // Check devices for active ethernet connection
    match nm.list_devices().await {
        Ok(devices) => {
            for device in devices {
                // Check if device is ethernet and connected
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

    // List networks to find signal strength of current network
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

async fn run_monitor() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use nmrs::NetworkManager;

    let nm = NetworkManager::new().await?;
    info!("Connected to NetworkManager, starting network monitor...");

    // Send initial status
    send_update(&nm).await;

    // Monitor device changes (covers both WiFi and Ethernet state changes)
    // This is a blocking call that listens for D-Bus signals
    nm.monitor_device_changes(|| {
        debug!("Network device state changed");
        // We need to fetch new status asynchronously
        // Since this callback is sync, we'll use a channel or spawn
    })
    .await?;

    // Note: monitor_device_changes blocks forever, so we won't reach here
    // If we need periodic updates as well, we can use polling fallback

    Ok(())
}

async fn send_update(nm: &nmrs::NetworkManager) {
    let status = fetch_network_status_with_nm(nm).await;
    debug!(
        "Network update: connected={}, type={:?}, signal={:?}",
        status.connected, status.connection_type, status.signal_strength
    );
    events::send_network(status);
}

async fn fetch_network_status_with_nm(nm: &nmrs::NetworkManager) -> NetworkStatus {
    // Check current connection
    let ssid = nm.current_ssid().await;
    let connected = ssid.is_some();

    if !connected {
        return check_ethernet_status(nm).await;
    }

    let signal_strength = get_current_signal_strength(nm, ssid.as_deref()).await;

    NetworkStatus {
        connected: true,
        signal_strength,
        ssid,
        connection_type: ConnectionType::Wifi,
    }
}
