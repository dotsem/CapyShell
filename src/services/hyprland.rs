use hyprland::event_listener::EventListener;
use log::{debug, error, info};
use std::thread;

mod active_window;
mod hotplug;
mod icon;
pub(crate) mod workspaces;

/// Number of workspaces to show per monitor.
const WORKSPACES_PER_MONITOR: i32 = 10;

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
            super::hyprland::hotplug::hotplug();
        });

        listener.add_monitor_removed_handler(|name| {
            debug!("Monitor removed: {}. Restarting to reconfigure...", name);
            thread::sleep(std::time::Duration::from_millis(200));
            super::hyprland::hotplug::hotplug();
        });

        // === Workspace event handlers ===
        listener.add_workspace_changed_handler(|ws| {
            debug!("Workspace changed event: {:?}", ws);
            workspaces::send_update_to_all_monitors();
        });

        listener.add_active_window_changed_handler(|win| {
            debug!("Active window changed: {:?}", win);
            workspaces::send_update_to_all_monitors();
        });

        listener.add_window_opened_handler(|win| {
            debug!("Window opened: {:?}", win);
            workspaces::send_update_to_all_monitors();
        });

        listener.add_window_closed_handler(|addr| {
            debug!("Window closed: {:?}", addr);
            workspaces::send_update_to_all_monitors();
        });

        listener.add_urgent_state_changed_handler(|addr| {
            debug!("Urgent state changed: {:?}", addr);
            workspaces::send_update_to_all_monitors();
        });

        info!("Hyprland event listener active (workspaces + hotplug)");
        if let Err(e) = listener.start_listener() {
            error!("Hyprland listener failed: {}", e);
        }
    });
}
