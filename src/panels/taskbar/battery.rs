//! Battery UI update handler for taskbar.
//!
//! Receives battery events from the service layer and updates the Slint UI.
//! The actual monitoring logic lives in services/battery.rs.

use crate::panels::taskbar::{BatteryData, BatteryState as SlintBatteryState, Taskbar};
use crate::services::battery::{BatteryState, BatteryStatus};

/// Convert service battery status to Slint UI data.
pub fn status_to_ui_data(status: &BatteryStatus) -> BatteryData {
    BatteryData {
        percentage: status.percentage,
        state: battery_state_to_slint(status.state),
        time_remaining: status.time_remaining.clone().into(),
    }
}

/// Update the taskbar UI with battery data.
pub fn update_ui(ui: &Taskbar, status: &BatteryStatus) {
    ui.set_battery_data(status_to_ui_data(status));
}

/// Convert service battery state to Slint enum.
fn battery_state_to_slint(state: BatteryState) -> SlintBatteryState {
    match state {
        BatteryState::Unknown => SlintBatteryState::Unknown,
        BatteryState::Full => SlintBatteryState::Full,
        BatteryState::Alert => SlintBatteryState::Alert,
        BatteryState::Bar0 => SlintBatteryState::Bar0,
        BatteryState::Bar1 => SlintBatteryState::Bar1,
        BatteryState::Bar2 => SlintBatteryState::Bar2,
        BatteryState::Bar3 => SlintBatteryState::Bar3,
        BatteryState::Bar4 => SlintBatteryState::Bar4,
        BatteryState::Bar5 => SlintBatteryState::Bar5,
        BatteryState::Bar6 => SlintBatteryState::Bar6,
        BatteryState::Charging20 => SlintBatteryState::Charging20,
        BatteryState::Charging30 => SlintBatteryState::Charging30,
        BatteryState::Charging80 => SlintBatteryState::Charging80,
        BatteryState::Charging90 => SlintBatteryState::Charging90,
        BatteryState::ChargingFull => SlintBatteryState::ChargingFull,
    }
}
