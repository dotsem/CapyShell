//! App catalog and icon service shim.
//!
//! Wraps the capy-apps crate to provide app metadata and icon lookup.

use capy_apps::{AppCatalog, DesktopApp};
use log::info;
use std::path::PathBuf;
use std::sync::Arc;

pub use capy_apps::DesktopApp as AppInfo;

/// Start background indexing of apps and icons.
/// Call this once from services::start_all().
pub fn start_indexing() {
    info!("Starting app catalog background indexing (capy-apps)...");

    let catalog = capy_apps::get_catalog();

    std::thread::spawn(move || {
        catalog.refresh();
        crate::services::wm::trigger_refresh();

        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            // catalog.refresh(); // Avoid constant scanning for now
        }
    });
}

/// Get icon path for an app class/name.
pub fn get_icon(name: &str) -> Option<PathBuf> {
    capy_apps::get_icon(name)
}

/// Get app by ID.
pub fn get_app(id: &str) -> Option<DesktopApp> {
    capy_apps::get_app(id)
}
