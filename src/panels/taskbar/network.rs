//! Network UI update handler for taskbar.
//!
//! Receives network events from the service layer and updates the Slint UI.
//! The actual monitoring logic lives in services/network.rs.

use crate::panels::taskbar::taskbar::{NetworkState as SlintNetworkState, Taskbar};
use crate::services::network::{ConnectionType, NetworkStatus};

/// Update the taskbar UI with network data.
pub fn update_ui(ui: &Taskbar, status: &NetworkStatus) {
    let state = status_to_slint_state(status);
    ui.set_network_state(state);
}

/// Convert service network status to Slint UI state.
fn status_to_slint_state(status: &NetworkStatus) -> SlintNetworkState {
    if !status.connected {
        return SlintNetworkState::NotConnected;
    }

    match status.connection_type {
        ConnectionType::Ethernet => SlintNetworkState::Lan,
        ConnectionType::Wifi => wifi_signal_to_state(status.signal_strength),
        ConnectionType::None => SlintNetworkState::NotConnected,
    }
}

/// Convert WiFi signal strength (0-100%) to Slint NetworkState.
fn wifi_signal_to_state(signal: Option<u8>) -> SlintNetworkState {
    match signal {
        None => SlintNetworkState::Connected0, // Unknown signal, show minimal
        Some(0..=20) => SlintNetworkState::Connected0,
        Some(21..=40) => SlintNetworkState::Connected1,
        Some(41..=60) => SlintNetworkState::Connected2,
        Some(61..=80) => SlintNetworkState::Connected3,
        Some(81..=100) => SlintNetworkState::Connected4,
        Some(_) => SlintNetworkState::Connected4, // > 100 (shouldn't happen)
    }
}
