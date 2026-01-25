use log::{debug, info};
use slint::Image;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::panels::taskbar::events::TaskbarEvent;

use crate::panel_manager::{PanelFactory, PanelInstance};
use crate::services;
use crate::services::wm::hyprland_wm;
use hyprland::data::Monitor;
use slint::ComponentHandle;
use spell_framework::layer_properties::{BoardType, LayerAnchor, LayerType, WindowConf};
use std::error::Error;
mod slint_taskbar {
    include!(concat!(env!("OUT_DIR"), "/taskbar.rs"));
    pub use slint_generatedTaskbar::*;
}
pub use self::slint_taskbar::*;

pub mod active_window;
pub mod battery;
pub mod bluetooth;
pub mod clock;
pub mod distro_icon;
pub mod events;
pub mod media;
pub mod network;
pub mod volume;
pub mod workspaces;

const TASKBAR_HEIGHT: u32 = 48; // TODO: make configurable
const EVENT_POLL_INTERVAL_MS: u64 = 50;

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

pub struct TaskbarFactory {
    has_battery: bool,
    has_bluetooth: bool,
}

impl TaskbarFactory {
    pub fn new(has_battery: bool, has_bluetooth: bool) -> Self {
        Self {
            has_battery,
            has_bluetooth,
        }
    }
}

impl PanelFactory for TaskbarFactory {
    fn type_id(&self) -> &str {
        "taskbar"
    }

    fn generate_configs(&self, monitors: &[Monitor]) -> Vec<(String, WindowConf, Monitor)> {
        monitors
            .iter()
            .map(|monitor| {
                let name = format!("taskbar-{}", monitor.name);
                let conf = WindowConf::new(
                    monitor.width as u32,
                    TASKBAR_HEIGHT,
                    (
                        Some(LayerAnchor::TOP | LayerAnchor::LEFT | LayerAnchor::RIGHT),
                        None,
                    ),
                    (0, 0, 0, 0),
                    LayerType::Top,
                    BoardType::None,
                    Some(TASKBAR_HEIGHT as i32),
                    Some(monitor.name.clone()),
                );
                (name, conf, monitor.clone())
            })
            .collect()
    }

    fn create_instance(
        &self,
        unique_name: &str,
        monitor: &Monitor,
    ) -> Result<Box<dyn PanelInstance>, Box<dyn Error>> {
        info!(
            "Creating Taskbar instance for monitor '{}' ({})",
            monitor.name, unique_name
        );

        let ui = Taskbar::new()?;
        let monitor_name = monitor.name.clone();

        // --- Setup Callbacks and Events (copied from main.rs) ---

        // Clock callback
        let ui_weak_clock = ui.as_weak();
        ui.on_update_clock(move || {
            if let Some(ui_handle) = ui_weak_clock.upgrade() {
                clock::update_clock(&ui_handle);
            }
        });

        // Workspace click callback
        ui.on_workspace_clicked(move |workspace_id| {
            workspaces::switch_to_workspace(workspace_id);
        });

        // Event polling timer
        let mut event_rx = events::subscribe();
        let ui_weak_events = ui.as_weak();
        let monitor_name_for_events = monitor_name.clone();

        let event_timer = slint::Timer::default();
        event_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(EVENT_POLL_INTERVAL_MS),
            move || {
                let events = events::drain_latest(&mut event_rx);
                if events.is_empty() {
                    return;
                }
                if let Some(ui) = ui_weak_events.upgrade() {
                    for event in events {
                        match event {
                            TaskbarEvent::Battery(status) => {
                                battery::update_ui(&ui, &status);
                            }
                            TaskbarEvent::Volume(status) => {
                                volume::update_ui(&ui, &status);
                            }
                            TaskbarEvent::Network(status) => {
                                network::update_ui(&ui, &status);
                            }
                            TaskbarEvent::Bluetooth(status) => {
                                bluetooth::update_ui(&ui, &status);
                            }
                            TaskbarEvent::Workspaces(status) => {
                                workspaces::update_ui(&ui, &status, &monitor_name_for_events);
                            }
                            TaskbarEvent::Mpris(data) => {
                                media::update_ui(&ui, &data);
                            }
                            TaskbarEvent::ActiveWindow(data) => {
                                // Logic for active window
                                active_window::update_ui(&ui, &data, &monitor_name_for_events);
                            }
                            TaskbarEvent::SystemStatus(_data) => {
                                // TODO: Implement UI update for system status
                            }
                        }
                    }
                }
            },
        );

        // init ui
        clock::update_clock(&ui);
        distro_icon::update_distro_icon(&ui);

        ui.set_has_battery(self.has_battery);
        if self.has_battery {
            let initial_battery = services::battery::get_status();
            battery::update_ui(&ui, &initial_battery);
        }

        let initial_volume = services::volume::get_default_volume();
        if let Some(vol) = initial_volume {
            volume::update_ui(&ui, &vol);
        }

        let initial_network = services::network::get_status();
        network::update_ui(&ui, &initial_network);

        ui.set_has_bluetooth(self.has_bluetooth);
        if self.has_bluetooth {
            let initial_bluetooth = services::bluetooth::get_status();
            bluetooth::update_ui(&ui, &initial_bluetooth);
        }

        let initial_workspaces = hyprland_wm::workspaces::get_status(&monitor_name);
        workspaces::update_ui(&ui, &initial_workspaces, &monitor_name);

        media::attach_callbacks(&ui);

        let initial_active_window = hyprland_wm::active_window::get_active_window();
        active_window::update_ui(&ui, &initial_active_window, &monitor_name);

        // Keep timer alive
        std::mem::forget(event_timer);

        Ok(Box::new(TaskbarInstance { _ui: ui }))
    }
}

struct TaskbarInstance {
    _ui: Taskbar,
}

impl PanelInstance for TaskbarInstance {}
