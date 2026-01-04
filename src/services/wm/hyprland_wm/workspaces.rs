use crate::panels::taskbar::events;
use crate::services::wm::hyprland_wm::WORKSPACES_PER_MONITOR;
use crate::services::wm::hyprland_wm::icon;
use crate::services::wm::{WorkspaceInfo, WorkspaceState, WorkspacesStatus};
use hyprland::data::{Client, Clients, Monitors, Workspaces};
use hyprland::shared::{HyprData, HyprDataVec};
use log::{debug, info};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};

/// Shared state for workspace tracking.
struct WorkspaceTracker {
    /// Map of monitor name -> monitor index (for workspace ID calculation).
    monitor_indices: HashMap<String, usize>,
    /// Cached icon paths by app class.
    icon_cache: HashMap<String, Option<PathBuf>>,
}

impl WorkspaceTracker {
    fn new() -> Self {
        Self {
            monitor_indices: HashMap::new(),
            icon_cache: HashMap::new(),
        }
    }

    /// Update monitor indices from current monitor list.
    fn refresh_monitors(&mut self) {
        use hyprland::data::Monitors;

        if let Ok(monitors) = Monitors::get() {
            self.monitor_indices.clear();
            for (idx, monitor) in monitors.iter().enumerate() {
                self.monitor_indices.insert(monitor.name.clone(), idx);
            }
            debug!("Refreshed monitors: {:?}", self.monitor_indices);
        }
    }

    /// Look up icon for an app class, with caching.
    fn get_icon(&mut self, app_class: &str) -> Option<PathBuf> {
        if let Some(cached) = self.icon_cache.get(app_class) {
            return cached.clone();
        }

        let icon_path = icon::lookup_icon(app_class);
        debug!("Icon lookup for '{}': {:?}", app_class, icon_path);
        self.icon_cache
            .insert(app_class.to_string(), icon_path.clone());
        icon_path
    }
}

/// Global tracker state.
static TRACKER: OnceLock<Arc<RwLock<WorkspaceTracker>>> = OnceLock::new();

fn get_tracker() -> Arc<RwLock<WorkspaceTracker>> {
    TRACKER
        .get_or_init(|| {
            let mut tracker = WorkspaceTracker::new();
            tracker.refresh_monitors();
            Arc::new(RwLock::new(tracker))
        })
        .clone()
}

/// Get current workspace status for a specific monitor.
pub fn get_status(monitor_name: &str) -> WorkspacesStatus {
    let tracker = get_tracker();
    let mut tracker = tracker.write().unwrap();

    // Get current hyprland state
    let all_monitors: Vec<_> = Monitors::get().map(|m| m.to_vec()).unwrap_or_default();
    let all_workspaces: Vec<_> = Workspaces::get().map(|ws| ws.to_vec()).unwrap_or_default();
    let all_clients: Vec<_> = Clients::get().map(|c| c.to_vec()).unwrap_or_default();

    // Find this monitor and its index (position in the list determines workspace range)
    let monitor_idx = all_monitors
        .iter()
        .position(|m| m.name == monitor_name)
        .unwrap_or(0) as i32;

    // Calculate workspace range for this monitor
    let ws_start = monitor_idx * WORKSPACES_PER_MONITOR + 1;
    let ws_end = ws_start + WORKSPACES_PER_MONITOR - 1;

    // Get the active workspace for THIS monitor
    let this_monitor = all_monitors.iter().find(|m| m.name == monitor_name);
    let visible_ws_on_monitor = this_monitor.map(|m| m.active_workspace.id);
    let is_focused_monitor = this_monitor.map(|m| m.focused).unwrap_or(false);

    // Build workspace info for this monitor's range
    let mut workspaces = Vec::with_capacity(WORKSPACES_PER_MONITOR as usize);

    for ws_id in ws_start..=ws_end {
        let relative_id = ws_id - ws_start + 1; // Display as 1-10

        // Find if this workspace exists in Hyprland
        let ws_data = all_workspaces.iter().find(|ws| ws.id == ws_id);
        let has_windows = ws_data.map(|ws| ws.windows > 0).unwrap_or(false);

        // Find client on this workspace for icon
        let client_on_ws: Option<&Client> = all_clients.iter().find(|c| c.workspace.id == ws_id);

        let (app_class, icon_path) = if let Some(client) = client_on_ws {
            let class = client.class.clone();
            let icon = tracker.get_icon(&class);
            (Some(class), icon)
        } else {
            (None, None)
        };

        // Determine state
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

/// Trigger a refresh of all workspace UIs.
/// Called after icon indexing completes to update icons.
pub(crate) fn trigger_refresh() {
    info!("Triggering workspace refresh for icon updates...");

    // Clear the icon cache so we pick up newly indexed icons
    {
        let tracker = get_tracker();
        let mut tracker = tracker.write().unwrap();
        tracker.icon_cache.clear();
    }

    send_workspace_update_to_all_monitors();
}

/// Send workspace updates to all monitors.
pub(super) fn send_workspace_update_to_all_monitors() {
    // Get monitor names directly from Hyprland (bypass tracker)
    let monitor_names: Vec<String> = Monitors::get()
        .map(|m| m.iter().map(|m| m.name.clone()).collect())
        .unwrap_or_default();

    if monitor_names.is_empty() {
        info!("No monitors found from Hyprland");
        return;
    }

    debug!("Sending workspace updates to monitors: {:?}", monitor_names);

    for monitor_name in monitor_names {
        let status = get_status(&monitor_name);
        debug!(
            "Sending update for monitor '{}': {} workspaces, visible={}",
            status.monitor_name,
            status.workspaces.len(),
            status
                .workspaces
                .iter()
                .find(|w| w.state == WorkspaceState::Active || w.state == WorkspaceState::Visible)
                .map(|w| w.id)
                .unwrap_or(-1)
        );
        events::send_workspaces(status);
    }
}
