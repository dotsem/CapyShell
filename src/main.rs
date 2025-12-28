//! CapyShell - Hyper-performant shell for Hyprland
//!
//! Single-process multi-window architecture with restart-on-hotplug.

mod event_bus;
mod panels;
mod services;

use hyprland::data::{Monitor, Monitors};
use hyprland::shared::HyprData;
use log::{debug, error, info, warn};
use panels::taskbar::events::TaskbarEvent;
use panels::taskbar::taskbar::Taskbar;
use panels::taskbar::{battery, bluetooth, clock, events, network, volume, workspaces};
use slint::ComponentHandle;
use spell_framework::{
    enchant_spells,
    layer_properties::{BoardType, LayerAnchor, LayerType, WindowConf},
    slint_adapter::SpellMultiWinHandler,
    wayland_adapter::SpellWin,
};
use std::error::Error;

const TASKBAR_HEIGHT: u32 = 48;
const EVENT_POLL_INTERVAL_MS: u64 = 50;

fn main() -> Result<(), Box<dyn Error>> {
    info!("Starting CapyShell...");

    // Start shared background services ONCE
    let service_status = services::start_all();

    // Get all monitors
    let monitors: Vec<Monitor> = match Monitors::get() {
        Ok(monitors) => monitors.iter().cloned().collect(),
        Err(e) => {
            error!("Failed to get monitors: {}", e);
            return Err(e.into());
        }
    };

    if monitors.is_empty() {
        warn!("No monitors found!");
        return Ok(());
    }

    debug!("Found {} monitors", monitors.len());

    // Create window configs for all monitors
    let configs: Vec<(String, WindowConf)> = monitors
        .iter()
        .map(|m| {
            let name = format!("taskbar-{}", m.name);
            let conf = WindowConf::new(
                m.width as u32,
                TASKBAR_HEIGHT,
                (
                    Some(LayerAnchor::TOP | LayerAnchor::LEFT | LayerAnchor::RIGHT),
                    None,
                ),
                (0, 0, 0, 0),
                LayerType::Top,
                BoardType::None,
                Some(TASKBAR_HEIGHT as i32),
                Some(m.name.clone()),
            );
            (name, conf)
        })
        .collect();

    // Convert to the format conjure_spells expects
    let configs_ref: Vec<(&str, WindowConf)> = configs
        .iter()
        .map(|(name, conf)| (name.as_str(), conf.clone()))
        .collect();

    // Create all windows at once (sets up shared Slint platform)
    let windows: Vec<SpellWin> = SpellMultiWinHandler::conjure_spells(configs_ref);

    debug!("Created {} windows", windows.len());

    // Now create Slint UIs for each window
    let mut uis: Vec<Taskbar> = Vec::new();
    for (i, _waywin) in windows.iter().enumerate() {
        let ui = Taskbar::new()?;
        let monitor_name = monitors[i].name.clone();
        info!("Taskbar {} assigned to monitor '{}'", i, monitor_name);

        // Clock callback
        let ui_weak_clock = ui.as_weak();
        ui.on_update_clock(move || {
            if let Some(ui_handle) = ui_weak_clock.upgrade() {
                clock::update_clock(&ui_handle);
            }
        });

        // Workspace click callback
        ui.on_workspace_clicked(move |workspace_id| {
            workspaces::switch_to_workspace(workspace_id);
        });

        // Event polling timer - each taskbar subscribes to the broadcast channel
        let mut event_rx = events::subscribe();
        let ui_weak_events = ui.as_weak();
        let monitor_name_for_events = monitor_name.clone();

        let event_timer = slint::Timer::default();
        event_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(EVENT_POLL_INTERVAL_MS),
            move || {
                let events = events::drain_latest(&mut event_rx);
                if events.is_empty() {
                    return;
                }
                if let Some(ui) = ui_weak_events.upgrade() {
                    for event in events {
                        match event {
                            TaskbarEvent::Battery(status) => {
                                battery::update_ui(&ui, &status);
                            }
                            TaskbarEvent::Volume(status) => {
                                volume::update_ui(&ui, &status);
                            }
                            TaskbarEvent::Network(status) => {
                                network::update_ui(&ui, &status);
                            }
                            TaskbarEvent::Bluetooth(status) => {
                                bluetooth::update_ui(&ui, &status);
                            }
                            TaskbarEvent::Workspaces(status) => {
                                workspaces::update_ui(&ui, &status, &monitor_name_for_events);
                            }
                        }
                    }
                }
            },
        );

        // Initial state
        clock::update_clock(&ui);

        // Battery setup (only if battery is present)
        ui.set_has_battery(service_status.has_battery);
        if service_status.has_battery {
            let initial_status = services::battery::get_status();
            battery::update_ui(&ui, &initial_status);
        }

        // Initial volume state
        if let Some(volume_status) = services::volume::get_default_volume() {
            volume::update_ui(&ui, &volume_status);
        }

        // Initial network state
        let initial_network = services::network::get_status();
        network::update_ui(&ui, &initial_network);

        // Bluetooth setup (only if bluetooth adapter is present)
        ui.set_has_bluetooth(service_status.has_bluetooth);
        if service_status.has_bluetooth {
            let initial_bluetooth = services::bluetooth::get_status();
            bluetooth::update_ui(&ui, &initial_bluetooth);
        }

        // Initial workspace state
        let initial_workspaces = services::workspaces::get_status(&monitor_name);
        workspaces::update_ui(&ui, &initial_workspaces, &monitor_name);

        // Keep timer alive
        std::mem::forget(event_timer);

        uis.push(ui);
        debug!("Initialized UI for monitor {}", i);
    }

    info!("CapyShell running with {} taskbars.", windows.len());

    // Run all windows in single-threaded event loop
    let num_windows = windows.len();
    let states: Vec<_> = (0..num_windows).map(|_| None).collect();
    let callbacks: Vec<_> = (0..num_windows).map(|_| None).collect();

    enchant_spells(windows, states, callbacks)?;

    Ok(())
}
