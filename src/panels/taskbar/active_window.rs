use crate::panels::taskbar::load_icon;
use crate::panels::taskbar::taskbar::{ActiveWindowData, Taskbar};
use crate::services::wm::types::ActiveWindowInfo;

pub fn update_ui(ui: &Taskbar, data: &ActiveWindowInfo) {
    ui.set_activeWindow(active_window_to_ui_data(data));
}

fn active_window_to_ui_data(data: &ActiveWindowInfo) -> ActiveWindowData {
    let icon = data
        .icon_path
        .as_ref()
        .and_then(|path| load_icon(path))
        .unwrap_or_default();

    ActiveWindowData {
        app: data.app.clone().into(),
        window_title: data.window_title.clone().into(),
        icon,
    }
}
