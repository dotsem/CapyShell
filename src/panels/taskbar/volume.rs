//! Volume UI update handler for taskbar.
//!
//! Receives volume events from the service layer and updates the Slint UI.
//! The actual monitoring logic lives in services/volume.rs.

use crate::panels::taskbar::Taskbar;
use crate::services::volume::VolumeStatus;

/// Update the taskbar UI with volume data.
pub fn update_ui(ui: &Taskbar, status: &VolumeStatus) {
    // Clamp to 0-100 for display (can exceed 100 if amplified)
    let display_volume = status.volume_percent.clamp(0, 100);

    // If muted, show 0
    let effective_volume = if status.muted { 0 } else { display_volume };

    ui.set_volume(effective_volume);
}
