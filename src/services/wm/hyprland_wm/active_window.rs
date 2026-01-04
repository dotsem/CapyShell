use hyprland::{
    data::Client,
    event_listener::{WindowEventData, WindowTitleEventData},
    shared::HyprDataActiveOptional,
};
use log::warn;
use std::sync::{LazyLock, RwLock};

use crate::{
    panels::taskbar::events,
    services::wm::{hyprland_wm, types::ActiveWindowInfo},
};

static ACTIVE_WINDOW: LazyLock<RwLock<ActiveWindowInfo>> =
    LazyLock::new(|| RwLock::new(ActiveWindowInfo::default()));

pub(crate) fn set_active_window(window: Option<WindowEventData>) {
    let mut write_guard = ACTIVE_WINDOW.write().unwrap();
    let current_address = &write_guard.address;

    match window {
        Some(w) if w.address.to_string() != *current_address => {
            let new_info = ActiveWindowInfo {
                address: w.address.to_string(),
                app: w.class,
                window_title: w.title,
                icon_path: None,
                focused_monitor: hyprland_wm::get_active_monitor(),
            };
            *write_guard = new_info;
            events::send_active_window(write_guard.clone());
        }
        None if !current_address.is_empty() => {
            *write_guard = ActiveWindowInfo::default();
            events::send_active_window(write_guard.clone());
        }
        _ => {}
    }
}

pub(crate) fn update_active_window(title_info: WindowTitleEventData) {
    // the address of an active window updates should never be None
    if title_info.address.to_string() != *(ACTIVE_WINDOW.read().unwrap()).address.to_string() {
        return;
    }
    let mut write_guard = ACTIVE_WINDOW.write().unwrap();
    write_guard.window_title = title_info.title;
    events::send_active_window(write_guard.clone());
}

pub(crate) fn init_active_window() {
    if let Ok(Some(active_window)) = Client::get_active() {
        let info = ActiveWindowInfo {
            address: active_window.address.to_string(),
            app: active_window.class,
            window_title: active_window.title,
            icon_path: None,
            focused_monitor: hyprland_wm::get_active_monitor(),
        };
        let mut write_guard = ACTIVE_WINDOW.write().unwrap();
        *write_guard = info;
    } else {
        warn!("Failed to initialize active window");
    }
}

pub fn get_active_window() -> ActiveWindowInfo {
    ACTIVE_WINDOW.read().unwrap().clone()
}
