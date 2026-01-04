use crate::panels::taskbar::load_icon;
use crate::panels::taskbar::taskbar::{ActiveWindowData, Taskbar};
use crate::services::wm::types::ActiveWindowInfo;

pub fn update_ui(ui: &Taskbar, data: &ActiveWindowInfo, monitor_name: &str) {
    ui.set_activeWindow(active_window_to_ui_data(data, monitor_name));
}

fn active_window_to_ui_data(data: &ActiveWindowInfo, monitor_name: &str) -> ActiveWindowData {
    let icon = data
        .icon_path
        .as_ref()
        .and_then(|path| load_icon(path))
        .unwrap_or_default();

    ActiveWindowData {
        app: data.app.clone().into(),
        window_title: data.window_title.clone().into(),
        icon,
        active_monitor: data.focused_monitor == monitor_name,
    }
}
