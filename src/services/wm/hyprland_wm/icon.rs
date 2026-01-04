//! Icon lookup for workspaces.
//!
//! TODO: generalize this to work with other Desktop Environments.

use std::path::PathBuf;

/// Look up icon for app class using the apps service.
pub(crate) fn lookup_icon(app_class: &str) -> Option<PathBuf> {
    crate::services::apps::get_icon(app_class)
}
