//! CapyShell - Hyper-performant shell for Hyprland
//!
//! Single-process multi-window architecture with restart-on-hotplug.

mod event_bus;
mod icons;
mod panels;

use hyprland::data::{Monitor, Monitors};
use hyprland::shared::HyprData;
use panels::taskbar::events::TaskbarEvent;
use panels::taskbar::taskbar::Taskbar;
use panels::taskbar::{battery, clock, events, hyprland_events};
use slint::ComponentHandle;
use spell_framework::{
    enchant_spells,
    layer_properties::{BoardType, LayerAnchor, WindowConf},
    slint_adapter::SpellMultiWinHandler,
    wayland_adapter::SpellWin,
};
use std::error::Error;

const TASKBAR_HEIGHT: u32 = 48;
const EVENT_POLL_INTERVAL_MS: u64 = 50;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting CapyShell...");

    // Start shared background services ONCE
    battery::start_battery_monitor();

    // Get all monitors
    let monitors: Vec<Monitor> = match Monitors::get() {
        Ok(monitors) => monitors.iter().cloned().collect(),
        Err(e) => {
            eprintln!("Failed to get monitors: {}", e);
            return Err(e.into());
        }
    };

    if monitors.is_empty() {
        eprintln!("No monitors found!");
        return Ok(());
    }

    println!("Found {} monitors", monitors.len());

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
                spell_framework::layer_properties::LayerType::Top,
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

    println!("Created {} windows", windows.len());

    // --- Icon Cache (Strings) ---
    // Using Symbols Nerd Font codepoints
    let battery_icons = icons::Icons::new();

    // Helper to get icon from state
    let get_icon = |state: panels::taskbar::taskbar::BatterState,
                    icons: &icons::Icons|
     -> slint::SharedString {
        use panels::taskbar::taskbar::BatterState;
        match state {
            BatterState::Unknown => icons.unknown.clone(),
            BatterState::Critical => icons.critical.clone(),
            BatterState::Low => icons.low.clone(),
            BatterState::S1 => icons.s1.clone(),
            BatterState::S2 => icons.s2.clone(),
            BatterState::S3 => icons.s3.clone(),
            BatterState::S4 => icons.s4.clone(),
            BatterState::S5 => icons.s5.clone(),
            BatterState::S6 => icons.s6.clone(),
            BatterState::Full => icons.full.clone(),
            BatterState::Charging => icons.charging.clone(),
            BatterState::ConnectedNotCharging => icons.charging.clone(),
            _ => icons.unknown.clone(),
        }
    };

    // Now create Slint UIs for each window
    let mut uis: Vec<Taskbar> = Vec::new();
    for (i, waywin) in windows.iter().enumerate() {
        let ui = Taskbar::new()?;

        // Setup input region
        let actual_size = ui.window().size();
        waywin.subtract_input_region(0, 0, actual_size.width as i32, actual_size.height as i32);

        // Clock callback
        let ui_weak_clock = ui.as_weak();
        ui.on_update_clock(move || {
            if let Some(ui_handle) = ui_weak_clock.upgrade() {
                clock::update_clock(&ui_handle);
            }
        });

        // Event polling timer
        let event_rx = events::receiver();
        let ui_weak_events = ui.as_weak();
        let icons_clone = battery_icons.clone();

        let event_timer = slint::Timer::default();
        event_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(EVENT_POLL_INTERVAL_MS),
            move || {
                let events = events::drain_latest(&event_rx);
                if events.is_empty() {
                    return;
                }
                if let Some(ui) = ui_weak_events.upgrade() {
                    for event in events {
                        match event {
                            TaskbarEvent::Battery(status) => {
                                // Map status -> view model using cached icons
                                let data = panels::taskbar::taskbar::BatteryData {
                                    percentage: status.percentage,
                                    state: status.state,
                                    icon: get_icon(status.state, &icons_clone),
                                };
                                ui.set_battery_state(data);
                            }
                        }
                    }
                }
            },
        );

        // Initial state
        clock::update_clock(&ui);

        // Initial battery
        let initial_status = battery::get_initial_battery_status();
        let initial_data = panels::taskbar::taskbar::BatteryData {
            percentage: initial_status.percentage,
            state: initial_status.state,
            icon: get_icon(initial_status.state, &battery_icons),
        };
        ui.set_battery_state(initial_data);

        // Keep timer alive
        std::mem::forget(event_timer);

        uis.push(ui);
        println!("Initialized UI for monitor {}", i);
    }

    // Start hotplug listener that triggers restart
    hyprland_events::start_restart_listener();

    println!("CapyShell running with {} taskbars.", windows.len());

    // Run all windows in single-threaded event loop
    let num_windows = windows.len();
    let states: Vec<_> = (0..num_windows).map(|_| None).collect();
    let callbacks: Vec<_> = (0..num_windows).map(|_| None).collect();

    enchant_spells(windows, states, callbacks)?;

    Ok(())
}
