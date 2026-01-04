use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum WorkspaceState {
    #[default]
    Empty,
    Visible,
    Active,
    Attention,
}

#[derive(Clone, Debug, Default)]
pub struct WorkspaceInfo {
    pub id: i32,
    pub absolute_id: i32,
    pub state: WorkspaceState,
    pub icon_path: Option<PathBuf>,
    pub occupied: bool,
    pub app_class: Option<String>,
}

#[derive(Clone, Debug)]
pub struct WorkspacesStatus {
    pub monitor_name: String,
    pub workspaces: Vec<WorkspaceInfo>,
}

// TODO: if not used in future, remove
#[derive(Clone, Debug, Default)]
pub struct WindowInfo {
    pub id: String,
    pub class: String,
    pub title: String,
    pub initial_class: String,
    pub workspace: String,
    pub is_active: bool,
    pub icon_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct ActiveWindowInfo {
    /// The address of the active window
    /// This is used to track if a title update belongs to the current active window
    /// We treat it as a string because address is basically just a string & doesn't have the [Default] implementation
    pub address: String,
    /// App name (e.g. "Firefox")
    pub app: String,
    /// Window title (e.g. "Rust (programming language) - Wikipedia - Mozilla Firefox")
    pub window_title: String,
    /// The path of the app icon for the active window
    pub icon_path: Option<PathBuf>,
    /// The name of the monitor where the active window is focused
    pub focused_monitor: String,
}
