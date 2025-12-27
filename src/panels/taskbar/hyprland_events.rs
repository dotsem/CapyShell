//! Hyprland event listener for monitor hotplug.
//!
//! When monitors are added or removed, triggers application restart
//! to recreate taskbars for the new monitor configuration.

use hyprland::event_listener::EventListener;
use std::thread;

use log::{error, info};
/// Start a listener that triggers application restart on monitor changes.
/// This allows the single-process architecture to handle hotplug by restarting.
use std::os::unix::process::CommandExt;
use std::process::Command;
/// Start a listener that triggers application restart on monitor changes.
/// This allows the single-process architecture to handle hotplug by restarting.
pub fn start_restart_listener() {
    thread::spawn(move || {
        let mut listener = EventListener::new();

        // On monitor added, trigger restart
        listener.add_monitor_added_handler(move |event_data| {
            info!(
                "Monitor added: {}. Restarting to reconfigure...",
                event_data.name
            );
            // Small delay to let Hyprland settle
            thread::sleep(std::time::Duration::from_millis(200));
            restart_process();
        });

        // On monitor removed, trigger restart
        listener.add_monitor_removed_handler(move |name| {
            info!("Monitor removed: {}. Restarting to reconfigure...", name);
            thread::sleep(std::time::Duration::from_millis(200));
            restart_process();
        });

        info!("Hyprland hotplug listener active (will restart on changes)");
        if let Err(e) = listener.start_listener() {
            error!("Hyprland event listener failed: {}", e);
        }
    });
}

/// Restart the current process by replacing it with a new instance.
fn restart_process() {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            error!("Failed to get current executable: {}", e);
            std::process::exit(1);
        }
    };

    info!("Restarting CapyShell...");

    // Preserve arguments (though currently we don't use any)
    let args: Vec<String> = std::env::args().skip(1).collect();

    let err = Command::new(exe).args(args).exec();

    // If we get here, exec failed
    error!("Failed to restart process: {}", err);
    std::process::exit(1);
}
