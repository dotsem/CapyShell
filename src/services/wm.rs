pub mod hyprland_wm;
pub mod types;

pub use types::{WindowInfo, WorkspaceInfo, WorkspaceState, WorkspacesStatus};

use std::sync::OnceLock;

static CURRENT_DE: OnceLock<String> = OnceLock::new();

pub fn get_current_de() -> &'static str {
    CURRENT_DE.get_or_init(|| "hyprland".to_string())
}

pub fn start_monitor() {
    match get_current_de() {
        "hyprland" => crate::services::wm::hyprland_wm::start_monitor(),
        de => log::warn!("Unsupported DE: {}", de),
    }
}

pub fn get_workspaces_status(monitor_name: &str) -> WorkspacesStatus {
    match get_current_de() {
        "hyprland" => crate::services::wm::hyprland_wm::workspaces::get_status(monitor_name),
        _ => WorkspacesStatus {
            monitor_name: monitor_name.to_string(),
            workspaces: Vec::new(),
        },
    }
}

pub fn trigger_refresh() {
    match get_current_de() {
        "hyprland" => crate::services::wm::hyprland_wm::workspaces::trigger_refresh(),
        _ => {}
    }
}
