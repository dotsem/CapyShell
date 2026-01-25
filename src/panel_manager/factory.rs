use crate::panel_manager::{PanelInstance, WindowConf};
use hyprland::data::Monitor;
use std::error::Error;

/// Trait that defines a factory for creating a specific type of window (e.g., Taskbar, Launcher).
pub trait PanelFactory {
    /// Unique identifier for this window type.
    fn type_id(&self) -> &str;

    /// Generates window configurations based on the available monitors.
    /// Returns a list of (UniqueName, WindowConfig, Monitor).
    fn generate_configs(&self, monitors: &[Monitor]) -> Vec<(String, WindowConf, Monitor)>;

    /// Initializes a window instance for a specific monitor and configuration.
    /// This is where the Slint UI is created and event handlers are attached.
    /// The `unique_name` matches the one provided in `generate_configs`.
    fn create_instance(
        &self,
        unique_name: &str,
        monitor: &Monitor,
    ) -> Result<Box<dyn PanelInstance>, Box<dyn Error>>;
}
