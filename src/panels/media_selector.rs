use std::error::Error;

use hyprland::data::Monitor;
use log::info;
use spell_framework::layer_properties::{BoardType, LayerAnchor, LayerType, WindowConf};

use crate::panel_manager::{PanelFactory, PanelInstance};
mod slint_media_selector {
    include!(concat!(env!("OUT_DIR"), "/media_selector.rs"));
    pub use slint_generatedMediaSelector::*;
}
pub use self::slint_media_selector::*;

pub struct MediaSelectorFactory {}

impl MediaSelectorFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl PanelFactory for MediaSelectorFactory {
    fn type_id(&self) -> &str {
        "music-player-selector"
    }

    fn generate_configs(&self, monitors: &[Monitor]) -> Vec<(String, WindowConf, Monitor)> {
        let mut configs = Vec::new();
        for monitor in monitors {
            configs.push((
                String::from("music-player-selector"),
                WindowConf::new(
                    300,
                    100,
                    (Some(LayerAnchor::TOP | LayerAnchor::LEFT), None),
                    (0, 0, 0, 300),
                    LayerType::Top,
                    BoardType::None,
                    None,
                    None,
                ),
                monitor.clone(),
            ));
        }
        configs
    }

    fn create_instance(
        &self,
        unique_name: &str,
        monitor: &Monitor,
    ) -> Result<Box<dyn PanelInstance>, Box<dyn Error>> {
        info!(
            "Creating MediaSelector instance for monitor '{}' ({})",
            monitor.name, unique_name
        );

        let ui = MediaSelector::new()?;

        Ok(Box::new(MediaSelectorInstance { _ui: ui }))
    }
}

struct MediaSelectorInstance {
    _ui: MediaSelector,
}

impl PanelInstance for MediaSelectorInstance {}
