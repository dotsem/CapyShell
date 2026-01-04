use log::debug;
use slint::Image;

pub mod active_window;
pub mod battery;
pub mod bluetooth;
pub mod clock;
pub mod events;
pub mod media;
pub mod network;
pub mod taskbar;
pub mod volume;
pub mod workspaces;

/// Load an icon from a file path into a Slint Image.
pub fn load_icon(path: &std::path::Path) -> Option<Image> {
    match Image::load_from_path(path) {
        Ok(image) => {
            debug!("Successfully loaded icon: {:?}", path);
            Some(image)
        }
        Err(e) => {
            debug!("Failed to load icon {:?}: {}", path, e);
            None
        }
    }
}
