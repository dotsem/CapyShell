use std::path::Path;

use log::{info, trace, warn};

use crate::services::system_info;

use super::taskbar::Taskbar;

pub fn update_distro_icon(ui: &Taskbar) {
    let distro_logo = format!(
        "assets/distro/{}.svg",
        system_info::get_static_info().distribution_id
    );
    trace!("distro_logo: {}", distro_logo);

    let path = Path::new(&distro_logo);
    match slint::Image::load_from_path(path) {
        Ok(image) => ui.set_distro_icon(image),
        Err(e) => {
            info!(
                "Failed to load specific distro icon, falling back to linux.svg: {}",
                e
            );
            let path = Path::new("assets/distro/linux.svg");
            match slint::Image::load_from_path(path) {
                Ok(image) => ui.set_distro_icon(image),
                Err(e) => warn!("Failed to load distro icon: {}", e),
            }
        }
    }
}
