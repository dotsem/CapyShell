//! Workspace management for Hyprland.

use crate::types::{WorkspaceInfo, WorkspaceState, WorkspacesStatus};
use crate::{WmEvent, resolve_icon, send_event};
use hyprland::data::{Clients, Monitors, Workspaces};
use hyprland::shared::{HyprData, HyprDataVec};
use log::debug;

use super::WORKSPACES_PER_MONITOR;

/// Get workspace status for a specific monitor.
pub fn get_status(monitor_name: &str) -> WorkspacesStatus {
    let all_monitors: Vec<_> = Monitors::get().map(|m| m.to_vec()).unwrap_or_default();
    let all_workspaces: Vec<_> = Workspaces::get().map(|ws| ws.to_vec()).unwrap_or_default();
    let all_clients: Vec<_> = Clients::get().map(|c| c.to_vec()).unwrap_or_default();

    let monitor_idx = all_monitors
        .iter()
        .position(|m| m.name == monitor_name)
        .unwrap_or(0) as i32;

    // workspace range for this monitor
    let ws_start = monitor_idx * WORKSPACES_PER_MONITOR + 1;
    let ws_end = ws_start + WORKSPACES_PER_MONITOR - 1;

    // active workspace for this monitor
    let this_monitor = all_monitors.iter().find(|m| m.name == monitor_name);
    let visible_ws_on_monitor = this_monitor.map(|m| m.active_workspace.id);

    let is_focused_monitor = this_monitor.map(|m| m.focused).unwrap_or(false);

    let mut workspaces = Vec::with_capacity(WORKSPACES_PER_MONITOR as usize);

    for ws_id in ws_start..=ws_end {
        let relative_id = ws_id - ws_start + 1;

        let ws_data = all_workspaces.iter().find(|ws| ws.id == ws_id);
        let has_windows = ws_data.map(|ws| ws.windows > 0).unwrap_or(false);

        let client_on_ws = all_clients.iter().find(|c| c.workspace.id == ws_id);

        let (app_class, icon_path) = if let Some(client) = client_on_ws {
            let class = client.class.clone();
            let icon = resolve_icon(&class);
            (Some(class), icon)
        } else {
            (None, None)
        };

        let state = if Some(ws_id) == visible_ws_on_monitor {
            if is_focused_monitor {
                WorkspaceState::Active
            } else {
                WorkspaceState::Visible
            }
        } else {
            WorkspaceState::Empty
        };

        workspaces.push(WorkspaceInfo {
            id: relative_id,
            absolute_id: ws_id,
            state,
            icon_path,
            occupied: has_windows,
            app_class,
        });
    }

    WorkspacesStatus {
        monitor_name: monitor_name.to_string(),
        workspaces,
    }
}

/// Send workspace updates to all monitors.
pub(crate) fn send_updates_to_all_monitors() {
    let monitor_names: Vec<String> = Monitors::get()
        .map(|m| m.iter().map(|m| m.name.clone()).collect())
        .unwrap_or_default();

    if monitor_names.is_empty() {
        debug!("No monitors found from Hyprland");
        return;
    }

    for monitor_name in monitor_names {
        let status = get_status(&monitor_name);
        send_event(WmEvent::WorkspacesChanged(status));
    }
}
