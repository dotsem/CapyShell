mod panels;

use panels::taskbar::run_taskbar;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    run_taskbar()?;

    // Future panels:
    // std::thread::spawn(|| panels::menu::run_menu().unwrap());
    // std::thread::spawn(|| panels::osd::run_osd().unwrap());

    Ok(())
}
