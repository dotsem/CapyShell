//! Workspace monitoring service using Hyprland IPC.
//!
//! Monitors workspace state and broadcasts changes to taskbar.
//! Each monitor displays workspaces in a fixed range (monitor 0: 1-10, monitor 1: 11-20, etc).

use crate::panels::taskbar::events;
use hyprland::data::{Client, Clients, Monitors, Workspaces};
use hyprland::event_listener::EventListener;
use hyprland::shared::{HyprData, HyprDataVec};
use log::{debug, error, info};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::thread;

/// Workspace state for UI display.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum WorkspaceState {
    #[default]
    Empty,
    Occupied,
    Visible, // Active on this monitor but monitor not focused
    Active,  // Active on focused monitor
    Attention,
}

/// Information about a single workspace.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceInfo {
    /// Workspace ID (1-10 relative to monitor) for display.
    pub id: i32,
    /// Absolute workspace ID for switching.
    pub absolute_id: i32,
    /// Current state.
    pub state: WorkspaceState,
    /// Path to app icon (if any windows).
    pub icon_path: Option<PathBuf>,
    /// App class for the focused window (for icon lookup).
    pub app_class: Option<String>,
}

/// Status update for a specific monitor's workspaces.
#[derive(Clone, Debug)]
pub struct WorkspacesStatus {
    /// Monitor name this update is for.
    pub monitor_name: String,
    /// Workspaces for this monitor (always 10).
    pub workspaces: Vec<WorkspaceInfo>,
}

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

        let icon_path = lookup_icon(app_class);
        debug!("Icon lookup for '{}': {:?}", app_class, icon_path);
        self.icon_cache
            .insert(app_class.to_string(), icon_path.clone());
        icon_path
    }
}

/// Global tracker state.
static TRACKER: std::sync::OnceLock<Arc<RwLock<WorkspaceTracker>>> = std::sync::OnceLock::new();

fn get_tracker() -> Arc<RwLock<WorkspaceTracker>> {
    TRACKER
        .get_or_init(|| {
            let mut tracker = WorkspaceTracker::new();
            tracker.refresh_monitors();
            Arc::new(RwLock::new(tracker))
        })
        .clone()
}

/// Trigger a refresh of all workspace UIs.
/// Called after icon indexing completes to update icons.
pub fn trigger_refresh() {
    info!("Triggering workspace refresh for icon updates...");

    // Clear the icon cache so we pick up newly indexed icons
    {
        let tracker = get_tracker();
        let mut tracker = tracker.write().unwrap();
        tracker.icon_cache.clear();
    }

    // Send updates to all monitors
    send_all_updates();
}

/// Number of workspaces to show per monitor.
const WORKSPACES_PER_MONITOR: i32 = 10;

/// Get current workspace status for a specific monitor.
/// Monitor 0 shows workspaces 1-10, Monitor 1 shows 11-20, etc.
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
        } else if has_windows {
            WorkspaceState::Occupied
        } else {
            WorkspaceState::Empty
        };

        workspaces.push(WorkspaceInfo {
            id: relative_id,    // Show 1-10 for display
            absolute_id: ws_id, // Actual ID (1-10 or 11-20) for clicking
            state,
            icon_path,
            app_class,
        });
    }

    WorkspacesStatus {
        monitor_name: monitor_name.to_string(),
        workspaces,
    }
}

/// Start the workspace monitoring background thread.
/// Also handles monitor hotplug to restart the application.
pub fn start_monitor() {
    info!("Starting workspace monitor...");

    thread::spawn(move || {
        let mut listener = EventListener::new();

        // === Monitor hotplug handlers (trigger restart) ===
        listener.add_monitor_added_handler(|event_data| {
            debug!(
                "Monitor added: {}. Restarting to reconfigure...",
                event_data.name
            );
            thread::sleep(std::time::Duration::from_millis(200));
            super::hyprland::restart_process();
        });

        listener.add_monitor_removed_handler(|name| {
            debug!("Monitor removed: {}. Restarting to reconfigure...", name);
            thread::sleep(std::time::Duration::from_millis(200));
            super::hyprland::restart_process();
        });

        // === Workspace event handlers ===
        listener.add_workspace_changed_handler(|ws| {
            debug!("Workspace changed event: {:?}", ws);
            send_all_updates();
        });

        listener.add_active_window_changed_handler(|win| {
            debug!("Active window changed: {:?}", win);
            send_all_updates();
        });

        listener.add_window_opened_handler(|win| {
            debug!("Window opened: {:?}", win);
            send_all_updates();
        });

        listener.add_window_closed_handler(|addr| {
            debug!("Window closed: {:?}", addr);
            send_all_updates();
        });

        listener.add_urgent_state_changed_handler(|addr| {
            debug!("Urgent state changed: {:?}", addr);
            send_all_updates();
        });

        info!("Hyprland event listener active (workspaces + hotplug)");
        if let Err(e) = listener.start_listener() {
            error!("Hyprland listener failed: {}", e);
        }
    });
}

/// Send workspace updates to all monitors.
fn send_all_updates() {
    // Get monitor names directly from Hyprland (bypass tracker)
    let monitor_names: Vec<String> = Monitors::get()
        .map(|m| m.iter().map(|m| m.name.clone()).collect())
        .unwrap_or_default();

    if monitor_names.is_empty() {
        info!("No monitors found from Hyprland");
        return;
    }

    info!("Sending workspace updates to monitors: {:?}", monitor_names);

    for monitor_name in monitor_names {
        let status = get_status(&monitor_name);
        info!(
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

/// Look up icon for app class using the apps service.
fn lookup_icon(app_class: &str) -> Option<PathBuf> {
    crate::services::apps::get_icon(app_class)
}
