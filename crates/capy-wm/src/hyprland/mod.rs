//! Hyprland backend for capy-wm.
//!
//! Implements the WindowBackend trait for the Hyprland compositor.

mod active_window;
mod workspaces;

use crate::{ActiveWindowInfo, WindowBackend, WmEvent, WorkspacesStatus, send_event};
use hyprland::data::Monitors;
use hyprland::dispatch::{Dispatch, DispatchType, WorkspaceIdentifierWithSpecial};
use hyprland::event_listener::EventListener;
use hyprland::shared::HyprData;
use log::{debug, error, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Number of workspaces per monitor.
pub const WORKSPACES_PER_MONITOR: i32 = 10;

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Hyprland window manager backend.
pub struct HyprlandBackend;

impl HyprlandBackend {
    /// Create a new Hyprland backend instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for HyprlandBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowBackend for HyprlandBackend {
    fn get_workspaces(&self, monitor_name: &str) -> WorkspacesStatus {
        workspaces::get_status(monitor_name)
    }

    fn get_active_window(&self) -> ActiveWindowInfo {
        active_window::get()
    }

    fn get_active_monitor(&self) -> String {
        Monitors::get()
            .ok()
            .and_then(|monitors| monitors.iter().find(|m| m.focused).map(|m| m.name.clone()))
            .unwrap_or_default()
    }

    fn get_monitors(&self) -> Vec<String> {
        Monitors::get()
            .map(|m| m.iter().map(|m| m.name.clone()).collect())
            .unwrap_or_default()
    }

    fn switch_workspace(&self, workspace_id: i32) {
        let _ = Dispatch::call(DispatchType::Workspace(WorkspaceIdentifierWithSpecial::Id(
            workspace_id,
        )));
    }

    fn start_listener(&self) {
        if RUNNING.swap(true, Ordering::SeqCst) {
            info!("Hyprland listener already running");
            return;
        }

        info!("Starting Hyprland event listener...");

        // Initialize active window state
        active_window::init();

        thread::spawn(move || {
            let mut listener = EventListener::new();

            // Monitor hotplug handlers
            listener.add_monitor_added_handler(|event_data| {
                debug!("Monitor added: {}", event_data.name);
                thread::sleep(std::time::Duration::from_millis(200));
                send_event(WmEvent::MonitorAdded(event_data.name));
            });

            listener.add_monitor_removed_handler(|name| {
                debug!("Monitor removed: {}", name);
                thread::sleep(std::time::Duration::from_millis(200));
                send_event(WmEvent::MonitorRemoved(name));
            });

            // Workspace event handlers
            listener.add_workspace_changed_handler(|ws| {
                debug!("Workspace changed event: {:?}", ws);
                workspaces::send_updates_to_all_monitors();
            });

            listener.add_active_window_changed_handler(|win| {
                debug!("Active window changed: {:?}", win);
                active_window::set(win);
                workspaces::send_updates_to_all_monitors();
            });

            listener.add_window_opened_handler(|win| {
                debug!("Window opened: {:?}", win);
                workspaces::send_updates_to_all_monitors();
            });

            listener.add_window_closed_handler(|addr| {
                debug!("Window closed: {:?}", addr);
                workspaces::send_updates_to_all_monitors();
            });

            listener.add_urgent_state_changed_handler(|addr| {
                debug!("Urgent state changed: {:?}", addr);
                workspaces::send_updates_to_all_monitors();
            });

            listener.add_window_title_changed_handler(|addr| {
                debug!("Window title changed: {:?}", addr.title);
                active_window::update_title(addr);
            });

            info!("Hyprland event listener active");
            if let Err(e) = listener.start_listener() {
                error!("Hyprland listener failed: {}", e);
                RUNNING.store(false, Ordering::SeqCst);
            }
        });
    }

    fn trigger_refresh(&self) {
        info!("Triggering Hyprland state refresh...");
        workspaces::send_updates_to_all_monitors();
        active_window::init();
        let active = active_window::get();
        send_event(WmEvent::ActiveWindowChanged(active));
    }

    fn init_active_window(&self) {
        active_window::init();
    }
}

// Re-export for backwards compatibility during transition
pub use active_window::get as get_active_window;
pub use workspaces::get_status as get_workspaces_status;

/// Get the active monitor name.
pub fn get_active_monitor() -> String {
    HyprlandBackend::new().get_active_monitor()
}

/// Trigger a refresh.
pub fn trigger_refresh() {
    HyprlandBackend::new().trigger_refresh()
}

/// Start the listener.
pub fn start_listener() {
    HyprlandBackend::new().start_listener()
}

// Legacy compatibility - these are used by the shim
pub fn set_icon_resolver(resolver: crate::IconResolver) {
    crate::set_icon_resolver(resolver);
}

pub fn set_event_callback<F>(callback: F)
where
    F: Fn(WmEvent) + Send + Sync + 'static,
{
    crate::set_event_callback(callback);
}
