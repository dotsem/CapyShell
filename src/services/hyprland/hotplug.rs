//! Hyprland utility functions for CapyShell.
//!
//! Monitor hotplug is now handled in the workspaces service to use a single thread.

use log::{error, info};
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Restarts the current process by replacing it with a new instance.
/// Used for monitor hotplug to recreate taskbars with dynamic monitor growth.
pub fn hotplug() {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            error!("Failed to get current executable: {}", e);
            std::process::exit(1);
        }
    };

    info!("Hotplugging CapyShell...");

    // Preserve arguments (though currently we don't use any)
    let args: Vec<String> = std::env::args().skip(1).collect();

    let err = Command::new(exe).args(args).exec();

    // If we get here, exec failed
    error!("Failed to hotplug CapyShell: {}", err);
    std::process::exit(1);
}
