//! Core types for window manager abstraction.

use std::path::PathBuf;

/// Detected window manager type.
/// More will come soon (define soon)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WmType {
    Hyprland,
    Sway,
    Niri,
    Unknown,
}

impl std::fmt::Display for WmType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WmType::Hyprland => write!(f, "Hyprland"),
            WmType::Sway => write!(f, "Sway"),
            WmType::Niri => write!(f, "Niri"),
            WmType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// State of a workspace in the taskbar.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum WorkspaceState {
    #[default]
    Empty,
    Visible,
    Active,
    Attention,
}

/// Information about a single workspace.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceInfo {
    /// Relative workspace ID (1-... for display).
    pub id: i32,
    /// Absolute workspace ID used by the WM.
    pub absolute_id: i32,
    /// Current state of the workspace.
    pub state: WorkspaceState,
    /// Path to the icon for the primary app on this workspace.
    pub icon_path: Option<PathBuf>,
    /// Whether the workspace has any windows.
    pub occupied: bool,
    /// App class of the primary window (if any).
    pub app_class: Option<String>,
}

/// Workspace status for a specific monitor.
#[derive(Clone, Debug)]
pub struct WorkspacesStatus {
    /// Name of the monitor.
    pub monitor_name: String,
    /// List of workspaces for this monitor.
    pub workspaces: Vec<WorkspaceInfo>,
}

impl Default for WorkspacesStatus {
    fn default() -> Self {
        Self {
            monitor_name: String::new(),
            workspaces: Vec::new(),
        }
    }
}

/// Information about a window.
#[derive(Clone, Debug, Default)]
pub struct WindowInfo {
    /// Unique window ID.
    pub id: String,
    /// Window class (app identifier).
    pub class: String,
    /// Window title.
    pub title: String,
    /// Initial class when the window was created.
    pub initial_class: String,
    /// Workspace the window is on.
    pub workspace: String,
    /// Whether this window is currently active.
    pub is_active: bool,
    /// Path to the app icon.
    pub icon_path: Option<PathBuf>,
}

/// Information about the currently active window.
#[derive(Clone, Debug, Default)]
pub struct ActiveWindowInfo {
    /// Unique address of the window.
    pub address: String,
    /// App name (e.g. "Firefox").
    pub app: String,
    /// Window title.
    pub window_title: String,
    /// Path to the app icon.
    pub icon_path: Option<PathBuf>,
    /// Name of the monitor where the window is focused.
    pub focused_monitor: String,
}

/// Events emitted by the window manager backend.
#[derive(Clone, Debug)]
pub enum WmEvent {
    /// Workspace state changed for a monitor.
    WorkspacesChanged(WorkspacesStatus),
    /// Active window changed.
    ActiveWindowChanged(ActiveWindowInfo),
    /// Monitor added, passes the name of the monitor that was added.
    MonitorAdded(String),
    /// Monitor removed, passes the name of the monitor that was removed.
    MonitorRemoved(String),
}
