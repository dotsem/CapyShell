//! CapyShell - Hyper-performant shell for Hyprland
//!
//! Single-process multi-window architecture with restart-on-hotplug.

mod event_bus;
mod functions;
mod panel_manager;
mod panels;
mod services;

use hyprland::data::{Monitor, Monitors};
use hyprland::shared::HyprData;
use log::{error, info};
use std::error::Error;

use panel_manager::PanelManager;

use crate::panels::media_selector::MediaSelectorFactory;
use crate::panels::taskbar::TaskbarFactory;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Welcome to CapyShell!"); // TODO: add more info

    let monitors: Vec<Monitor> = match Monitors::get() {
        Ok(monitors) => monitors.iter().cloned().collect(),
        Err(e) => {
            error!("Failed to get monitors: {}", e);
            return Err(e.into());
        }
    };

    // Start background services once before init of panels
    let service_status = services::start_all();

    let mut wm = PanelManager::new();

    let taskbar_factory =
        TaskbarFactory::new(service_status.has_battery, service_status.has_bluetooth);

    let media_selector_factory = MediaSelectorFactory::new();

    wm.register_factory(taskbar_factory);
    wm.register_factory(media_selector_factory);

    wm.start(&monitors)?;

    Ok(())
}
