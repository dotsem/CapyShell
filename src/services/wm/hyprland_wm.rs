use hyprland::{data::Monitors, event_listener::EventListener, shared::HyprData};
use log::{debug, error, info};
use std::thread;

use crate::{
    panels::taskbar::events,
    services::wm::hyprland_wm::workspaces::send_workspace_update_to_all_monitors,
};

pub(crate) mod active_window;
pub(crate) mod hotplug;
pub mod workspaces;

pub(crate) const WORKSPACES_PER_MONITOR: i32 = 10;

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
            super::hyprland_wm::hotplug::hotplug();
        });

        listener.add_monitor_removed_handler(|name| {
            debug!("Monitor removed: {}. Restarting to reconfigure...", name);
            thread::sleep(std::time::Duration::from_millis(200));
            super::hyprland_wm::hotplug::hotplug();
        });

        // === Workspace event handlers ===
        listener.add_workspace_changed_handler(|ws| {
            debug!("Workspace changed event: {:?}", ws);
            workspaces::send_workspace_update_to_all_monitors();
        });

        listener.add_active_window_changed_handler(|win| {
            info!("Active window changed: {:?}", win);
            active_window::set_active_window(win);

            workspaces::send_workspace_update_to_all_monitors();
        });

        listener.add_window_opened_handler(|win| {
            debug!("Window opened: {:?}", win);
            workspaces::send_workspace_update_to_all_monitors();
        });

        listener.add_window_closed_handler(|addr| {
            debug!("Window closed: {:?}", addr);
            workspaces::send_workspace_update_to_all_monitors();
        });

        listener.add_urgent_state_changed_handler(|addr| {
            debug!("Urgent state changed: {:?}", addr);
            workspaces::send_workspace_update_to_all_monitors()
        });

        listener.add_window_title_changed_handler(|addr| {
            info!("Window title changed: {:?}", addr.title);
            active_window::update_active_window(addr);
        });

        info!("Hyprland event listener active (workspaces + hotplug)");
        if let Err(e) = listener.start_listener() {
            error!("Hyprland listener failed: {}", e);
        }
    });
}

pub fn get_active_monitor() -> String {
    Monitors::get()
        .ok()
        .and_then(|monitors| monitors.iter().find(|m| m.focused).map(|m| m.name.clone()))
        .unwrap_or_default()
}

/// Trigger a refresh of all workspace UIs.
/// Called after icon indexing completes to update icons.
pub(crate) fn trigger_refresh() {
    info!("Triggering workspace refresh for icon updates...");

    // Clear the icon cache so we pick up newly indexed icons
    send_workspace_update_to_all_monitors();
    active_window::init_active_window();
    let active_window = active_window::get_active_window();
    events::send_active_window(active_window);
}
