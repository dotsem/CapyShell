//! Battery monitoring service.
//!
//! Listens to UPower D-Bus signals and sends BatteryData events
//! to the taskbar event bus.

use crate::panels::taskbar::events;
use crate::panels::taskbar::taskbar::BatteryState;
use battery::{Battery, Manager, State};
use std::thread;

/// Thread-safe battery status for cross-thread communication.
#[derive(Clone, Debug)]
pub struct BatteryStatus {
    pub percentage: i32,
    pub state: BatteryState,
    pub time_remaining: String,
}

/// Determine the battery state based on current status.
fn determine_battery_state(battery: &Battery) -> BatteryState {
    let percentage = get_percentage(battery);
    let state = battery.state();

    match state {
        State::Charging => charging_state_from_percentage(percentage),
        State::Full => BatteryState::Full,
        State::Discharging => discharging_state_from_percentage(percentage),
        State::Empty => BatteryState::Alert,
        State::Unknown => handle_unknown_state(battery, percentage),
        _ => BatteryState::Unknown,
    }
}

#[inline]
fn get_percentage(battery: &Battery) -> i32 {
    (battery.state_of_charge().value * 100.0) as i32
}

/// Map percentage to discharging battery icon states.
fn discharging_state_from_percentage(percentage: i32) -> BatteryState {
    match percentage {
        0..=4 => BatteryState::Alert,
        5..=14 => BatteryState::Bar0,
        15..=29 => BatteryState::Bar1,
        30..=44 => BatteryState::Bar2,
        45..=59 => BatteryState::Bar3,
        60..=74 => BatteryState::Bar4,
        75..=89 => BatteryState::Bar5,
        90..=99 => BatteryState::Bar6,
        _ => BatteryState::Full,
    }
}

/// Map percentage to charging battery icon states.
fn charging_state_from_percentage(percentage: i32) -> BatteryState {
    match percentage {
        0..=25 => BatteryState::Charging20,
        26..=50 => BatteryState::Charging30,
        51..=75 => BatteryState::Charging80,
        76..=99 => BatteryState::Charging90,
        _ => BatteryState::ChargingFull,
    }
}

/// Handle unknown battery state by checking energy rate.
fn handle_unknown_state(battery: &Battery, percentage: i32) -> BatteryState {
    let energy_rate = battery.energy_rate().value;

    if energy_rate > 0.0 {
        charging_state_from_percentage(percentage)
    } else if percentage >= 95 {
        BatteryState::Full
    } else {
        discharging_state_from_percentage(percentage)
    }
}

/// Format time remaining as "H:MM" string.
fn format_time_remaining(battery: &Battery) -> String {
    let state = battery.state();

    let duration = match state {
        State::Charging => battery.time_to_full(),
        State::Discharging => battery.time_to_empty(),
        _ => None,
    };

    match duration {
        Some(time) => {
            let total_minutes = (time.value / 60.0) as i32;
            let hours = total_minutes / 60;
            let minutes = total_minutes % 60;

            let suffix = if state == State::Charging {
                "until charged"
            } else {
                "remaining"
            };

            format!("{}:{:02} {}", hours, minutes, suffix)
        }
        None => String::new(),
    }
}

fn default_battery_status() -> BatteryStatus {
    BatteryStatus {
        percentage: 0,
        state: BatteryState::Unknown,
        time_remaining: String::new(),
    }
}

/// Get current battery data from system.
pub fn get_battery_status() -> BatteryStatus {
    match Manager::new() {
        Ok(manager) => get_battery_from_manager(manager),
        Err(e) => {
            eprintln!("Failed to create battery manager: {}", e);
            default_battery_status()
        }
    }
}

fn get_battery_from_manager(manager: Manager) -> BatteryStatus {
    match manager.batteries() {
        Ok(mut batteries) => match batteries.next() {
            Some(Ok(mut battery)) => {
                if let Err(e) = manager.refresh(&mut battery) {
                    eprintln!("Failed to refresh battery: {}", e);
                    return default_battery_status();
                }

                let percentage = get_percentage(&battery);
                let state = determine_battery_state(&battery);
                let time_remaining = format_time_remaining(&battery);

                BatteryStatus {
                    percentage,
                    state,
                    time_remaining,
                }
            }
            Some(Err(e)) => {
                eprintln!("Error reading battery: {}", e);
                default_battery_status()
            }
            None => {
                eprintln!("No battery found");
                default_battery_status()
            }
        },
        Err(e) => {
            eprintln!("Failed to get batteries: {}", e);
            default_battery_status()
        }
    }
}

/// Update the UI with current battery info (initial load).
pub fn get_initial_battery_status() -> BatteryStatus {
    get_battery_status()
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
    let status = get_battery_status();
    events::send_battery(status);
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
