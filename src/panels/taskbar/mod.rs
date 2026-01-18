use log::debug;
use slint::Image;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

pub mod active_window;
pub mod battery;
pub mod bluetooth;
pub mod clock;
pub mod distro_icon;
pub mod events;
pub mod media;
pub mod network;
pub mod taskbar;
pub mod volume;
pub mod workspaces;

// Thread-local icon cache to prevent repeated loading and memory growth
thread_local! {
    static ICON_CACHE: RefCell<HashMap<PathBuf, Image>> = RefCell::new(HashMap::new());
}

/// Load an icon from a file path into a Slint Image.
/// Uses a cache to prevent repeated loading of the same icons.
#[inline]
pub fn load_icon(path: &std::path::Path) -> Option<Image> {
    let path_buf = path.to_path_buf();

    // Check cache first
    ICON_CACHE.with(|cache| {
        let cache_borrow = cache.borrow();
        if let Some(img) = cache_borrow.get(&path_buf) {
            return Some(img.clone());
        }
        drop(cache_borrow); // Release borrow before mutating

        // Load and cache
        match Image::load_from_path(path) {
            Ok(image) => {
                debug!("Loaded and cached icon: {:?}", path);
                cache.borrow_mut().insert(path_buf, image.clone());
                Some(image)
            }
            Err(e) => {
                debug!("Failed to load icon {:?}: {}", path, e);
                None
            }
        }
    })
}
