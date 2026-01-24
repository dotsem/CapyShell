//! Window manager service shim.
//!
//! This module wraps the capy-wm crate and bridges it to CapyShell's event system.
//! It uses the unified WindowBackend trait for WM-agnostic operation.

use crate::panels::taskbar::events;
use log::info;
use std::sync::OnceLock;

// Re-export types from capy-wm
pub use capy_wm::{
    ActiveWindowInfo, WindowBackend, WmEvent, WmType, WorkspaceInfo, WorkspaceState,
    WorkspacesStatus,
};

// Global backend instance
static BACKEND: OnceLock<Box<dyn WindowBackend>> = OnceLock::new();

fn get_backend() -> &'static dyn WindowBackend {
    BACKEND.get_or_init(|| capy_wm::get_backend()).as_ref()
}

/// Re-export for backwards compatibility with existing code.
pub mod hyprland_wm {
    use super::*;

    pub mod workspaces {
        use super::*;

        pub fn get_status(monitor_name: &str) -> WorkspacesStatus {
            ::capy_wm::get_workspaces_status(monitor_name)
        }
    }

    pub mod active_window {
        use super::*;

        pub fn init_active_window() {
            get_backend().init_active_window();
        }

        pub fn get_active_window() -> ActiveWindowInfo {
            ::capy_wm::get_active_window()
        }
    }
}

/// Detect the current window manager.
pub fn detect_wm() -> WmType {
    capy_wm::detect_wm()
}

/// Start the window manager monitoring.
/// Sets up callbacks to bridge capy-wm events to CapyShell's event bus.
pub fn start_monitor() {
    let wm = detect_wm();
    info!("Starting WM service (detected: {})...", wm);

    // Set up icon resolver callback
    capy_wm::set_icon_resolver(Box::new(|class| crate::services::apps::get_icon(class)));

    // Set up event callback to bridge to CapyShell's event bus
    capy_wm::set_event_callback(|event| match event {
        WmEvent::WorkspacesChanged(status) => {
            events::send_workspaces(status);
        }
        WmEvent::ActiveWindowChanged(info) => {
            events::send_active_window(info);
        }
        WmEvent::MonitorAdded(_) | WmEvent::MonitorRemoved(_) => {
            hotplug();
        }
    });

    get_backend().start_listener();
}

/// Switch to a workspace by absolute ID.
pub fn switch_workspace(id: i32) {
    get_backend().switch_workspace(id);
}

/// Trigger a refresh of WM state (after icon indexing, etc.).
pub fn trigger_refresh() {
    get_backend().trigger_refresh();
}

/// Handle monitor hotplug by restarting the application.
/// This can later be handled differently, but i don't bother yet.
fn hotplug() {
    use log::error;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            error!("Failed to get current executable: {}", e);
            std::process::exit(1);
        }
    };

    info!("Hotplugging CapyShell...");

    let args: Vec<String> = std::env::args().skip(1).collect();
    let err = Command::new(exe).args(args).exec();

    error!("Failed to hotplug CapyShell: {}", err);
    std::process::exit(1);
}
