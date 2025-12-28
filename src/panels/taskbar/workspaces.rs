//! Workspace UI update handler for taskbar.
//!
//! Receives workspace events from the service layer and updates the Slint UI.

use crate::panels::taskbar::taskbar::{
    Taskbar, WorkspaceData, WorkspaceState as SlintWorkspaceState,
};
use crate::services::workspaces::{WorkspaceInfo, WorkspaceState, WorkspacesStatus};
use log::debug;
use slint::{Image, ModelRc, VecModel};
use std::rc::Rc;

/// Update the taskbar UI with workspace data.
/// Only updates if the event is for this taskbar's monitor.
pub fn update_ui(ui: &Taskbar, status: &WorkspacesStatus, monitor_name: &str) {
    // Filter by monitor - only handle events for this taskbar's monitor
    if status.monitor_name != monitor_name {
        return;
    }

    let workspace_data: Vec<WorkspaceData> = status
        .workspaces
        .iter()
        .map(|ws| workspace_to_slint(ws))
        .collect();

    let model: Rc<VecModel<WorkspaceData>> = Rc::new(VecModel::from(workspace_data));
    ui.set_workspaces(ModelRc::from(model));
}

/// Convert service workspace info to Slint WorkspaceData.
fn workspace_to_slint(ws: &WorkspaceInfo) -> WorkspaceData {
    let state = match ws.state {
        WorkspaceState::Empty => SlintWorkspaceState::Empty,
        WorkspaceState::Occupied => SlintWorkspaceState::Occupied,
        WorkspaceState::Visible => SlintWorkspaceState::Visible,
        WorkspaceState::Active => SlintWorkspaceState::Active,
        WorkspaceState::Attention => SlintWorkspaceState::Attention,
    };

    // Load icon if path exists
    let icon = ws
        .icon_path
        .as_ref()
        .and_then(|path| load_icon(path))
        .unwrap_or_default();

    WorkspaceData {
        id: ws.id,
        absolute_id: ws.absolute_id,
        state,
        icon,
    }
}

/// Load an icon from a file path into a Slint Image.
fn load_icon(path: &std::path::Path) -> Option<Image> {
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

/// Switch to the specified workspace.
pub fn switch_to_workspace(workspace_id: i32) {
    use hyprland::dispatch::{Dispatch, DispatchType, WorkspaceIdentifierWithSpecial};
    use log::debug;

    debug!("Switching to workspace {}", workspace_id);
    let _ = Dispatch::call(DispatchType::Workspace(WorkspaceIdentifierWithSpecial::Id(
        workspace_id,
    )));
}
