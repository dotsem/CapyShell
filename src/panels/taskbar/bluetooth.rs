//! Bluetooth UI update handler for taskbar.
//!
//! Receives bluetooth events from the service layer and updates the Slint UI.
//! The actual monitoring logic lives in services/bluetooth.rs.

use crate::panels::taskbar::{BluetoothState as SlintBluetoothState, Taskbar};
use crate::services::bluetooth::BluetoothStatus;

/// Update the taskbar UI with bluetooth data.
pub fn update_ui(ui: &Taskbar, status: &BluetoothStatus) {
    let state = status_to_slint_state(status);
    ui.set_bluetooth_state(state);
}

/// Convert service bluetooth status to Slint UI state.
fn status_to_slint_state(status: &BluetoothStatus) -> SlintBluetoothState {
    if !status.powered {
        return SlintBluetoothState::Off;
    }

    if status.connected_devices > 0 {
        return SlintBluetoothState::Connected;
    }

    if status.discovering {
        return SlintBluetoothState::Searching;
    }

    SlintBluetoothState::On
}
