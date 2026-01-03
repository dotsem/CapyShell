//! Shared system services for CapyShell.
//!
//! Services monitor system state and broadcast events to all panels.
//! Each service spawns a single background thread that all panels share.
//!
//! - `apps` - App catalog and icon lookup with caching
//! - `workspaces` - Workspace state, app icons, and monitor hotplug
//! - `volume` - PulseAudio/PipeWire volume monitoring
//! - `battery` - Battery status via D-Bus
//! - `network` - Network status via NetworkManager D-Bus
//! - `bluetooth` - Bluetooth status via BlueZ D-Bus
//! - `hyprland` - Utility functions (restart on hotplug)

pub mod apps;
pub mod battery;
pub mod bluetooth;
pub mod hyprland;
pub mod media;
pub mod network;
pub mod volume;

use log::info;

/// Start all shared background services.
/// Call this once from main before creating any panels.
pub fn start_all() -> ServiceStatus {
    info!("Starting shared services...");

    // Start app catalog indexing FIRST (non-blocking, runs in background)
    apps::start_indexing();

    let has_battery = battery::start_monitor();
    media::start();
    volume::start_monitor();
    network::start_monitor();
    let has_bluetooth = bluetooth::start_monitor();
    crate::services::hyprland::start_monitor(); // Also handles monitor hotplug

    ServiceStatus {
        has_battery,
        has_bluetooth,
    }
}

/// Status of started services, useful for conditional UI.
pub struct ServiceStatus {
    pub has_battery: bool,
    pub has_bluetooth: bool,
}
