use crate::WmType;
use crate::hyprland;

/// Trait that all window manager backends must implement.
/// This provides a unified interface regardless of the underlying WM.
pub trait WindowBackend: Send + Sync {
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
        if desktop_lower.contains("niri") {
            return WmType::Niri;
        }
    }

    // Specific Wm checks
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return WmType::Hyprland;
    }

    if std::env::var("SWAYSOCK").is_ok() {
        return WmType::Sway;
    }

    // TODO: check for niri
    // if std::env::var("NIRISOCK").is_ok() {
    //     return WmType::Niri;
    // }

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
        // WmType::Niri => Some(Box::new(niri::NiriBackend::new())),
        _ => None,
    }
}

/// Get the backend, panicking if no supported WM is detected.
/// Use this when you require a WM to be present.
pub fn get_backend() -> Box<dyn WindowBackend> {
    create_backend().expect("No supported window manager detected")
}
