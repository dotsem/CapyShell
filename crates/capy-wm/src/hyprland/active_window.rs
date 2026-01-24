//! Active window tracking for Hyprland.

use crate::types::ActiveWindowInfo;
use crate::{WmEvent, get_state, resolve_icon, send_event};
use hyprland::data::{Client, Monitors};
use hyprland::event_listener::{WindowEventData, WindowTitleEventData};
use hyprland::shared::{HyprData, HyprDataActiveOptional};
use log::warn;

fn get_active_monitor_name() -> String {
    Monitors::get()
        .ok()
        .and_then(|monitors| monitors.iter().find(|m| m.focused).map(|m| m.name.clone()))
        .unwrap_or_default()
}

/// Initialize active window state from current Hyprland state.
pub(crate) fn init() {
    if let Ok(Some(active)) = Client::get_active() {
        let info = ActiveWindowInfo {
            address: active.address.to_string(),
            app: active.class.clone(),
            window_title: active.title,
            icon_path: resolve_icon(&active.class),
            focused_monitor: get_active_monitor_name(),
        };
        send_event(WmEvent::ActiveWindowChanged(info));
    } else {
        warn!("Failed to initialize active window");
    }
}

/// Set the active window from a Hyprland event.
pub(crate) fn set(window: Option<WindowEventData>) {
    let current_address = match get_state().active_window.read() {
        Ok(guard) => guard.address.clone(),
        Err(_) => String::new(),
    };

    match window {
        Some(w) if w.address.to_string() != current_address => {
            let new_info = ActiveWindowInfo {
                address: w.address.to_string(),
                app: w.class.clone(),
                window_title: w.title,
                icon_path: resolve_icon(&w.class),
                focused_monitor: get_active_monitor_name(),
            };
            send_event(WmEvent::ActiveWindowChanged(new_info));
        }
        None if !current_address.is_empty() => {
            send_event(WmEvent::ActiveWindowChanged(ActiveWindowInfo::default()));
        }
        _ => {}
    }
}

/// Update the active window title.
pub(crate) fn update_title(title_info: WindowTitleEventData) {
    let mut info = match get_state().active_window.read() {
        Ok(guard) => {
            if title_info.address.to_string() != guard.address {
                return;
            }
            guard.clone()
        }
        Err(_) => return,
    };

    info.window_title = title_info.title;
    send_event(WmEvent::ActiveWindowChanged(info));
}

/// Get the current active window info.
pub fn get() -> ActiveWindowInfo {
    crate::get_active_window()
}
