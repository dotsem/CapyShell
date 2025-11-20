use slint::ComponentHandle;
use spell_framework::{
    cast_spell,
    layer_properties::{BoardType, LayerAnchor, WindowConf},
    wayland_adapter::SpellWin,
};
use std::{error::Error};
use hyprland::shared::HyprData;

use super::clock;
use super::battery;

// Include the Slint generated code
slint::include_modules!();

pub fn run_taskbar() -> Result<(), Box<dyn Error>> {
    // Try to get the actual screen width from Hyprland, fallback to default
    let screen_width = match hyprland::data::Monitors::get() {
        Ok(monitors) => {
            let active_monitor = monitors.iter().find(|m| m.focused).unwrap();
            active_monitor.width as u32
        }
        Err(e) => {
            eprintln!("Warning: Could not get Hyprland monitor info: {}. Using default width.", e);
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
        (0, 0, 0, 0), // No margins
        spell_framework::layer_properties::LayerType::Top,
        BoardType::None, 
        Some(taskbar_height as i32),             
    );

    let waywin = SpellWin::invoke_spell("taskbar", window_conf);
    let ui = Taskbar::new()?;
    
    // Get the actual window size that the Wayland compositor assigned
    // With TOP+LEFT+RIGHT anchors, this should be the full screen width
    let actual_size = ui.window().size();
    let final_width = actual_size.width;
    let final_height = actual_size.height;
        
    // Subtract input region using the actual window dimensions
    waywin.subtract_input_region(0, 0, final_width as i32, final_height as i32);

    // Create a shared System instance to avoid memory leaks


    // Set up the clock update callback
    let ui_weak_clock = ui.as_weak();
    ui.on_update_clock(move || {
        if let Some(ui_handle) = ui_weak_clock.upgrade() {
            clock::update_clock(&ui_handle);
        }
    });

    // Set up battery monitor
    let ui_weak_battery = ui.as_weak();
    battery::start_battery_monitor(ui_weak_battery);

    // Initial updates
    clock::update_clock(&ui);
    battery::update_battery_info(&ui);

    // Run the event loop through spell_framework
    cast_spell(waywin, None, None)
}
