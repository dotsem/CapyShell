use hyprland::data::Monitors;


pub fn init_taskbars() {
    let monitors = hyprland::data::Monitors::get();
    if let Ok(monitors) = monitors {
        for monitor in monitors {
            std::thread::spawn(move || {
                panels::taskbar::run_taskbar(monitor).unwrap();
            });
        }
    }
}