//! capy-apps: App catalog and icon resolver for Linux desktops.
//!
//! Provides a unified service for:
//! - Icon lookup with comprehensive directory scanning and theme inheritance (scans everything, if not let me know)
//! - Desktop application catalog parsing from .desktop files
//! - Caching for fast lookups
//! - Icon caching to disk for fast initial lookup

mod catalog;
mod desktop_entry;
mod icons;
mod paths;

pub use catalog::{AppCatalog, AppEvent};
pub use desktop_entry::DesktopApp;
pub use icons::IconTheme;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

static CATALOG: OnceLock<Arc<AppCatalog>> = OnceLock::new();

/// Get the global app catalog instance.
pub fn get_catalog() -> Arc<AppCatalog> {
    CATALOG
        .get_or_init(|| {
            let catalog = AppCatalog::new();
            catalog.refresh();
            Arc::new(catalog)
        })
        .clone()
}

/// Convenience function to look up an icon by name.
pub fn get_icon(name: &str) -> Option<PathBuf> {
    get_catalog().resolve_icon(name)
}

/// Convenience function to get app by ID (e.g. "firefox.desktop").
pub fn get_app(id: &str) -> Option<DesktopApp> {
    get_catalog().get_app(id)
}
