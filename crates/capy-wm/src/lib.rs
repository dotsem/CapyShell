//! capy-wm: Window manager abstraction for tiling window managers
//!
//! Provides a unified interface for interacting with tiling window managers.
//! Currently supports Hyprland with architecture ready for Sway, i3, etc.

pub mod types;

#[cfg(feature = "hyprland")]
pub mod hyprland;

pub use types::*;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

/// Icon resolver callback type.
/// The WM backend uses this to resolve app class names to icon paths.
pub type IconResolver = Box<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>;

/// Event callback type.
/// Called when WM events occur (workspace changes, active window changes, etc.).
pub type EventCallback = Box<dyn Fn(WmEvent) + Send + Sync>;

// Global callbacks shared across the backend
static ICON_RESOLVER: OnceLock<Arc<RwLock<Option<IconResolver>>>> = OnceLock::new();
static EVENT_CALLBACK: OnceLock<Arc<RwLock<Option<EventCallback>>>> = OnceLock::new();

fn get_icon_resolver_store() -> Arc<RwLock<Option<IconResolver>>> {
    ICON_RESOLVER
        .get_or_init(|| Arc::new(RwLock::new(None)))
        .clone()
}

fn get_event_callback_store() -> Arc<RwLock<Option<EventCallback>>> {
    EVENT_CALLBACK
        .get_or_init(|| Arc::new(RwLock::new(None)))
        .clone()
}

/// Set the icon resolver callback.
pub fn set_icon_resolver(resolver: IconResolver) {
    if let Ok(mut guard) = get_icon_resolver_store().write() {
        *guard = Some(resolver);
    }
}

/// Set the event callback.
pub fn set_event_callback<F>(callback: F)
where
    F: Fn(WmEvent) + Send + Sync + 'static,
{
    if let Ok(mut guard) = get_event_callback_store().write() {
        *guard = Some(Box::new(callback));
    }
}

/// Resolve an icon path using the configured resolver.
pub fn resolve_icon(class: &str) -> Option<PathBuf> {
    get_icon_resolver_store()
        .read()
        .ok()
        .and_then(|guard| guard.as_ref().and_then(|r| r(class)))
}

/// Send a WM event to the configured callback.
pub fn send_event(event: WmEvent) {
    if let Ok(guard) = get_event_callback_store().read() {
        if let Some(ref callback) = *guard {
            callback(event);
        }
    }
}

/// Trait that all window manager backends must implement.
/// This provides a unified interface regardless of the underlying WM.
pub trait WindowBackend: Send + Sync {
    /// Get workspace status for a specific monitor.
    fn get_workspaces(&self, monitor_name: &str) -> WorkspacesStatus;

    /// Get information about the currently active window.
    fn get_active_window(&self) -> ActiveWindowInfo;

    /// Get the name of the currently focused monitor.
    fn get_active_monitor(&self) -> String;

    /// Get all monitor names.
    fn get_monitors(&self) -> Vec<String>;

    /// Switch to a specific workspace by absolute ID.
    fn switch_workspace(&self, workspace_id: i32);

    /// Start the background event listener.
    /// This spawns a thread that monitors WM events and calls the event callback.
    fn start_listener(&self);

    /// Trigger a refresh of all WM state.
    fn trigger_refresh(&self);

    /// Initialize the active window state.
    fn init_active_window(&self);
}

/// Detect the current window manager from environment variables.
pub fn detect_wm() -> WmType {
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        let desktop_lower = desktop.to_lowercase();
        if desktop_lower.contains("hyprland") {
            return WmType::Hyprland;
        }
        if desktop_lower.contains("sway") {
            return WmType::Sway;
        }
        if desktop_lower.contains("i3") {
            return WmType::I3;
        }
    }

    // Specific Wm checks
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return WmType::Hyprland;
    }

    if std::env::var("SWAYSOCK").is_ok() {
        return WmType::Sway;
    }

    if std::env::var("I3SOCK").is_ok() {
        return WmType::I3;
    }

    WmType::Unknown
}

/// Create the appropriate backend for the detected window manager.
/// Returns None if no supported WM is detected.
pub fn create_backend() -> Option<Box<dyn WindowBackend>> {
    match detect_wm() {
        #[cfg(feature = "hyprland")]
        WmType::Hyprland => Some(Box::new(hyprland::HyprlandBackend::new())),

        // Future backends:
        // WmType::Sway => Some(Box::new(sway::SwayBackend::new())),
        // WmType::I3 => Some(Box::new(i3::I3Backend::new())),
        _ => None,
    }
}

/// Get the backend, panicking if no supported WM is detected.
/// Use this when you require a WM to be present.
pub fn get_backend() -> Box<dyn WindowBackend> {
    create_backend().expect("No supported window manager detected")
}
