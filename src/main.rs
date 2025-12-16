mod event_bus;
mod panels;

use panels::taskbar::run_taskbar;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    run_taskbar()?;

    // Future panels (each runs in its own thread with its own event loop):
    // std::thread::spawn(|| panels::sidebar::run_sidebar().unwrap());
    // std::thread::spawn(|| panels::menu::run_menu().unwrap());

    Ok(())
}
