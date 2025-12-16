use material_design_icons::{
    BATTERY, BATTERY_1_0, BATTERY_2_0, BATTERY_4_0, BATTERY_5_0, BATTERY_7_0, BATTERY_8_0,
    BATTERY_ALERT, BATTERY_CHARGING, BATTERY_UNKNOWN,
};

#[derive(Clone)]
pub struct Icons {
    pub unknown: slint::SharedString,
    pub critical: slint::SharedString,
    pub low: slint::SharedString,
    pub s1: slint::SharedString,
    pub s2: slint::SharedString,
    pub s3: slint::SharedString,
    pub s4: slint::SharedString,
    pub s5: slint::SharedString,
    pub s6: slint::SharedString,
    pub full: slint::SharedString,
    pub charging: slint::SharedString,
}

impl Icons {
    pub fn new() -> Self {
        Self {
            unknown: BATTERY_UNKNOWN.into(),
            critical: BATTERY_ALERT.into(),
            low: BATTERY_1_0.into(),
            s1: BATTERY_2_0.into(),
            s2: BATTERY_4_0.into(),
            s3: BATTERY_5_0.into(),
            s4: BATTERY_7_0.into(),
            s5: BATTERY_8_0.into(),
            s6: BATTERY.into(),
            full: BATTERY.into(),
            charging: BATTERY_CHARGING.into(),
        }
    }
}
