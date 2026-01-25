use hyprland::data::Monitor;
use log::{debug, info};
use spell_framework::{
    layer_properties::WindowConf, slint_adapter::SpellMultiWinHandler, wayland_adapter::SpellWin,
};
use std::error::Error;

pub mod factory;

pub use factory::PanelFactory;

/// Trait representing a tangible window instance (mostly for keeping the UI handle alive).
pub trait PanelInstance {
    fn on_show(&self) {}
    fn on_hide(&self) {}
}

/// The main manager that coordinates all windows.
pub struct PanelManager {
    factories: Vec<Box<dyn PanelFactory>>,
}

impl PanelManager {
    pub fn new() -> Self {
        Self {
            factories: Vec::new(),
        }
    }

    pub fn register_factory<F: PanelFactory + 'static>(&mut self, factory: F) {
        self.factories.push(Box::new(factory));
    }

    // Split the run to easier testing maybe? or just fix the ownership.
}

/// Helper to run the main loop, we might need to expose a method that takes the instances
/// and the windows and runs them.
/// But `PanelManager` holds factories, it shouldn't hold the runtime state necessarily forever?
/// Actually `run` should block.
///
/// Let's refine `run`.
impl PanelManager {
    // ... (new, register_factory)

    pub fn start(&self, monitors: &[Monitor]) -> Result<(), Box<dyn Error>> {
        // 1. Configuration Phase
        let mut factory_configs: Vec<(&Box<dyn PanelFactory>, Vec<(String, WindowConf, Monitor)>)> =
            Vec::new();

        for factory in &self.factories {
            let configs = factory.generate_configs(monitors);
            if !configs.is_empty() {
                factory_configs.push((factory, configs));
            }
        }

        let mut flat_configs: Vec<(&str, WindowConf)> = Vec::new();
        for (_, configs) in &factory_configs {
            for (name, conf, _) in configs {
                flat_configs.push((name.as_str(), conf.clone()));
            }
        }

        // 2. Window Creation Phase
        let windows: Vec<SpellWin> = SpellMultiWinHandler::conjure_spells(flat_configs.clone());

        // 3. UI Initialization Phase
        // We match them by order.
        let mut ui_instances: Vec<Box<dyn PanelInstance>> = Vec::new();

        // We iterate through our factory config groupings, they should match the flattened list structure.
        for (factory, configs) in &factory_configs {
            for (name, _, monitor) in configs {
                let instance = factory.create_instance(name, monitor)?;
                ui_instances.push(instance);
            }
        }

        // 4. Event Loop Phase
        let num_windows = windows.len();
        let states: Vec<_> = (0..num_windows).map(|_| None).collect();
        let callbacks: Vec<_> = (0..num_windows).map(|_| None).collect();

        use spell_framework::enchant_spells;
        // imports needed

        enchant_spells(windows, states, callbacks)?;

        Ok(())
    }
}
