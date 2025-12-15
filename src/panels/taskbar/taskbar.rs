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

slint::include_modules!();

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
    let final_width = actual_size.width;
    let final_height = actual_size.height;

    waywin.subtract_input_region(0, 0, final_width as i32, final_height as i32);

    // Clock update callback (triggered by Slint Timer in .slint file)
    let ui_weak_clock = ui.as_weak();
    ui.on_update_clock(move || {
        if let Some(ui_handle) = ui_weak_clock.upgrade() {
            clock::update_clock(&ui_handle);
        }
    });

    // Battery update callback (manual trigger)
    let ui_weak_battery = ui.as_weak();
    ui.on_update_battery(move || {
        if let Some(ui_handle) = ui_weak_battery.upgrade() {
            battery::update_battery_info(&ui_handle);
        }
    });

    // Start battery background monitor (D-Bus listener)
    battery::start_battery_monitor();

    // Set up Timer to poll battery channel and update UI
    let battery_rx = battery::get_battery_receiver();
    let ui_weak_poll = ui.as_weak();
    let battery_poll_timer = slint::Timer::default();
    battery_poll_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            // Drain all pending updates, keep only the latest
            let mut latest: Option<BatteryData> = None;
            while let Ok(data) = battery_rx.try_recv() {
                latest = Some(data);
            }

            if let Some(data) = latest {
                if let Some(ui_handle) = ui_weak_poll.upgrade() {
                    ui_handle.set_battery_state(data);
                }
            }
        },
    );

    // Initial updates
    clock::update_clock(&ui);
    battery::update_battery_info(&ui);

    // Keep timer alive by moving it into the event loop
    let _battery_timer = battery_poll_timer;

    // Run the event loop through spell_framework
    cast_spell(waywin, None, None)
}
