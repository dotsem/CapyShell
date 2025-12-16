//! Battery monitoring service.
//!
//! Listens to UPower D-Bus signals and sends BatteryData events
//! to the taskbar event bus.

use crate::panels::taskbar::events;
use crate::panels::taskbar::taskbar::{BatterState, BatteryData};
use battery::{Battery, Manager, State};
use std::thread;

fn determine_battery_state(battery: &Battery) -> BatterState {
    let percentage = get_percentage(battery);
    let state = battery.state();

    match state {
        State::Charging => BatterState::Charging,
        State::Full => BatterState::Full,
        State::Discharging => state_from_percentage(percentage),
        State::Empty => BatterState::Critical,
        State::Unknown => handle_unknown_state(battery, percentage),
        _ => BatterState::Unknown,
    }
}

#[inline]
fn get_percentage(battery: &Battery) -> i32 {
    (battery.state_of_charge().value * 100.0) as i32
}

fn state_from_percentage(percentage: i32) -> BatterState {
    match percentage {
        0..=4 => BatterState::Critical,
        5..=14 => BatterState::Low,
        15..=29 => BatterState::S1,
        30..=39 => BatterState::S2,
        40..=49 => BatterState::S3,
        50..=59 => BatterState::S4,
        60..=79 => BatterState::S5,
        80..=94 => BatterState::S6,
        _ => BatterState::Full,
    }
}

fn handle_unknown_state(battery: &Battery, percentage: i32) -> BatterState {
    let energy_rate = battery.energy_rate().value;

    if energy_rate > 0.0 {
        BatterState::Charging
    } else if percentage >= 95 {
        BatterState::Full
    } else {
        state_from_percentage(percentage)
    }
}

fn get_icon_for_state(state: BatterState) -> &'static str {
    match state {
        BatterState::Unknown => "󰂑",
        BatterState::Critical => "󰂃",
        BatterState::Low => "󰁺",
        BatterState::S1 => "󰁻",
        BatterState::S2 => "󰁼",
        BatterState::S3 => "󰁽",
        BatterState::S4 => "󰁾",
        BatterState::S5 => "󰁿",
        BatterState::S6 => "󰂀",
        BatterState::Full => "󰁹",
        BatterState::Charging => "󰂄",
        BatterState::ConnectedNotCharging => "󰂃",
        _ => "󰂑",
    }
}

fn default_battery_data() -> BatteryData {
    BatteryData {
        percentage: 0,
        state: BatterState::Unknown,
        icon: "󰂑".into(),
    }
}

/// Get current battery data from system.
pub fn get_battery_data() -> BatteryData {
    match Manager::new() {
        Ok(manager) => get_battery_data_from_manager(manager),
        Err(e) => {
            eprintln!("Failed to create battery manager: {}", e);
            default_battery_data()
        }
    }
}

fn get_battery_data_from_manager(manager: Manager) -> BatteryData {
    match manager.batteries() {
        Ok(mut batteries) => match batteries.next() {
            Some(Ok(mut battery)) => {
                if let Err(e) = manager.refresh(&mut battery) {
                    eprintln!("Failed to refresh battery: {}", e);
                    return default_battery_data();
                }

                let percentage = get_percentage(&battery);
                let state = determine_battery_state(&battery);
                let icon = get_icon_for_state(state);

                BatteryData {
                    percentage,
                    state,
                    icon: icon.into(),
                }
            }
            Some(Err(e)) => {
                eprintln!("Error reading battery: {}", e);
                default_battery_data()
            }
            None => {
                eprintln!("No battery found");
                default_battery_data()
            }
        },
        Err(e) => {
            eprintln!("Failed to get batteries: {}", e);
            default_battery_data()
        }
    }
}

/// Update the UI with current battery info (for initial load).
pub fn update_battery_info(ui: &crate::panels::taskbar::taskbar::Taskbar) {
    ui.set_battery_state(get_battery_data());
}

/// Start the battery monitoring background thread.
/// Sends events via the taskbar event bus.
pub fn start_battery_monitor() {
    println!("Starting battery monitor...");

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            if let Err(e) = dbus_worker().await {
                eprintln!("D-Bus worker failed: {}. Falling back to polling.", e);
                polling_worker().await;
            }
        });
    });
}

/// Send battery update to event bus.
#[inline]
fn send_battery_update() {
    let data = get_battery_data();
    events::send_battery(data);
}

async fn dbus_worker() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::stream::StreamExt;
    use zbus::{Connection, proxy};

    #[proxy(
        interface = "org.freedesktop.DBus.Properties",
        default_service = "org.freedesktop.UPower",
        default_path = "/org/freedesktop/UPower/devices/DisplayDevice"
    )]
    trait UPowerDevice {
        #[zbus(signal)]
        fn properties_changed(
            &self,
            interface_name: &str,
            changed_properties: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            invalidated_properties: Vec<String>,
        ) -> zbus::Result<()>;
    }

    let connection = Connection::system().await?;
    let proxy = UPowerDeviceProxy::new(&connection).await?;
    let mut properties_changed = proxy.receive_properties_changed().await?;

    println!("Listening for UPower D-Bus signals...");

    while let Some(signal) = properties_changed.next().await {
        if signal.args().is_ok() {
            send_battery_update();
        }
    }

    eprintln!("D-Bus signal stream ended");
    Ok(())
}

async fn polling_worker() {
    use tokio::time::{Duration, sleep};

    println!("Using polling fallback (every 30 seconds)");
    loop {
        sleep(Duration::from_secs(30)).await;
        send_battery_update();
    }
}
