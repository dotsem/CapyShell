use crate::panels::taskbar::Taskbar;

pub fn update_clock(ui: &Taskbar) {
    let now = chrono::Local::now();
    let time = now.format("%H:%M:%S").to_string();
    let date = now.format("%d/%m/%Y").to_string();

    let mut clock_state = ui.get_clock_state();
    clock_state.time = time.into();
    clock_state.date = date.into();
    ui.set_clock_state(clock_state);
}
