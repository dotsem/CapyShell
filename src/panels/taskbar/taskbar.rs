//! Taskbar panel implementation.
//!
//! Uses a single Timer to poll all background service events
//! for maximum efficiency.

use hyprland::shared::HyprData;
use slint::ComponentHandle;
use spell_framework::{
    cast_spell,
    layer_properties::{BoardType, LayerAnchor, WindowConf},
    wayland_adapter::SpellWin,
};
use std::error::Error;

use super::battery;
use super::clock;
use super::events::{self, TaskbarEvent};

slint::include_modules!();

/// Polling interval for the event bus Timer.
/// 50ms = 20Hz update rate. Fast enough for responsive UI,
/// slow enough to not waste CPU cycles.
const EVENT_POLL_INTERVAL_MS: u64 = 50;

pub fn run_taskbar() -> Result<(), Box<dyn Error>> {
    let screen_width = match hyprland::data::Monitors::get() {
        Ok(monitors) => {
            let active_monitor = monitors.iter().find(|m| m.focused).unwrap();
            active_monitor.width as u32
        }
        Err(e) => {
            eprintln!(
                "Warning: Could not get Hyprland monitor info: {}. Using default width.",
                e
            );
            1920u32
        }
    };
    let taskbar_height: u32 = 48;

    let window_conf = WindowConf::new(
        screen_width,
        taskbar_height,
        (
            Some(LayerAnchor::TOP | LayerAnchor::LEFT | LayerAnchor::RIGHT),
            None,
        ),
        (0, 0, 0, 0),
        spell_framework::layer_properties::LayerType::Top,
        BoardType::None,
        Some(taskbar_height as i32),
    );

    let waywin = SpellWin::invoke_spell("taskbar", window_conf);
    let ui = Taskbar::new()?;

    let actual_size = ui.window().size();
    waywin.subtract_input_region(0, 0, actual_size.width as i32, actual_size.height as i32);

    // Clock callback (triggered by Slint Timer in .slint file)
    let ui_weak_clock = ui.as_weak();
    ui.on_update_clock(move || {
        if let Some(ui_handle) = ui_weak_clock.upgrade() {
            clock::update_clock(&ui_handle);
        }
    });

    // Start background services
    battery::start_battery_monitor();

    // Single Timer polls ALL service events efficiently
    let event_rx = events::receiver();
    let ui_weak_events = ui.as_weak();
    let event_timer = slint::Timer::default();
    event_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(EVENT_POLL_INTERVAL_MS),
        move || {
            // Drain and deduplicate events (keeps latest per variant)
            let events = events::drain_latest(&event_rx);

            if events.is_empty() {
                return;
            }

            if let Some(ui) = ui_weak_events.upgrade() {
                for event in events {
                    match event {
                        TaskbarEvent::Battery(data) => {
                            ui.set_battery_state(data);
                        } // Future events:
                          // TaskbarEvent::Music(data) => ui.set_music_state(data),
                          // TaskbarEvent::Volume(data) => ui.set_volume_state(data),
                    }
                }
            }
        },
    );

    // Initial state
    clock::update_clock(&ui);
    battery::update_battery_info(&ui);

    // Keep timer alive
    let _event_timer = event_timer;

    cast_spell(waywin, None, None)
}
