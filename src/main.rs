mod panels;

use std::error::Error;
use panels::taskbar::run_taskbar;

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize and run all panels
    run_taskbar()?;
    
    // When you add more panels, start them here:
    // std::thread::spawn(|| {
    //     panels::menu::run_menu().unwrap();
    // });
    // std::thread::spawn(|| {
    //     panels::osd::run_osd().unwrap();
    // });
    
    Ok(())
}

// use hyprland::data::*;
// use hyprland::prelude::*;

// fn main() {
//     let monitors = Monitors::get();
//     println!("{monitors:#?}");

//     // let workspaces = Workspaces::get();
//     // println!("{workspaces:#?}");

//     // let clients = Clients::get();
//     // println!("{clients:#?}");

//     // let active_window = Client::get_active();
//     // println!("{active_window:#?}");

//     // let layers = Layers::get();
//     // println!("{layers:#?}");

//     // let devices = Devices::get();
//     // println!("{devices:#?}");

//     // let version = Version::get();
//     // println!("{version:#?}");

//     // let cursor_pos = CursorPosition::get();
//     // println!("{cursor_pos:#?}");

// }