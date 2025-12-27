//! Battery monitoring service.
//!
//! Listens to UPower D-Bus signals and broadcasts battery status events.
//! All panels receive updates through the taskbar event bus.

use crate::panels::taskbar::events;
use battery::{Battery, Manager, State};
use log::{error, info, warn};
use std::thread;

/// Battery state enum matching Slint UI expectations.
/// This is re-exported for UI layers to use.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BatteryState {
    #[default]
    Unknown,
    Full,
    // Discharging states
    Alert,
    Bar0,
    Bar1,
    Bar2,
    Bar3,
    Bar4,
    Bar5,
    Bar6,
    // Charging states
    Charging20,
    Charging30,
    Charging80,
    Charging90,
    ChargingFull,
}

/// Thread-safe battery status for cross-thread communication.
#[derive(Clone, Debug)]
pub struct BatteryStatus {
    pub percentage: i32,
    pub state: BatteryState,
    pub time_remaining: String,
}

/// Check if the system has any batteries available.
pub fn has_battery() -> bool {
    Manager::new()
        .ok()
        .and_then(|m| m.batteries().ok())
        .map(|mut b| b.next().is_some())
        .unwrap_or(false)
}

/// Get current battery data from system.
pub fn get_status() -> BatteryStatus {
    match Manager::new() {
        Ok(manager) => get_battery_from_manager(manager),
        Err(e) => {
            error!("Failed to create battery manager: {}", e);
            default_status()
        }
    }
}

/// Start the battery monitoring background thread.
/// Returns true if a battery was found and monitoring started.
pub fn start_monitor() -> bool {
    if !has_battery() {
        info!("No battery detected, skipping battery monitor");
        return false;
    }

    info!("Starting battery monitor...");

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            if let Err(e) = dbus_worker().await {
                warn!("D-Bus worker failed: {}. Falling back to polling.", e);
                polling_worker().await;
            }
        });
    });

    true
}

// === Internal implementation ===

fn default_status() -> BatteryStatus {
    BatteryStatus {
        percentage: 0,
        state: BatteryState::Unknown,
        time_remaining: String::new(),
    }
}

fn get_battery_from_manager(manager: Manager) -> BatteryStatus {
    match manager.batteries() {
        Ok(mut batteries) => match batteries.next() {
            Some(Ok(mut battery)) => {
                if let Err(e) = manager.refresh(&mut battery) {
                    error!("Failed to refresh battery: {}", e);
                    return default_status();
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
                error!("Error reading battery: {}", e);
                default_status()
            }
            None => {
                error!("No battery found");
                default_status()
            }
        },
        Err(e) => {
            error!("Failed to get batteries: {}", e);
            default_status()
        }
    }
}

#[inline]
fn get_percentage(battery: &Battery) -> i32 {
    (battery.state_of_charge().value * 100.0) as i32
}

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

fn charging_state_from_percentage(percentage: i32) -> BatteryState {
    match percentage {
        0..=25 => BatteryState::Charging20,
        26..=50 => BatteryState::Charging30,
        51..=75 => BatteryState::Charging80,
        76..=99 => BatteryState::Charging90,
        _ => BatteryState::ChargingFull,
    }
}

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

/// Send battery update to event bus.
#[inline]
fn send_update() {
    let status = get_status();
    events::send_battery(status);
}

async fn dbus_worker() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::stream::StreamExt;
    use zbus::Connection;

    let connection = Connection::system().await?;

    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .path_namespace("/org/freedesktop/UPower/devices")?
        .build();

    let mut stream = zbus::MessageStream::for_match_rule(rule, &connection, Some(100)).await?;

    info!("Listening for battery D-Bus signals...");

    while let Some(msg) = stream.next().await {
        if let Ok(msg) = msg {
            if let Some(path) = msg.header().path() {
                let path_str = path.as_str();
                if path_str.contains("battery_BAT") {
                    send_update();
                }
            }
        }
    }

    info!("D-Bus signal stream ended");
    Ok(())
}

async fn polling_worker() {
    use tokio::time::{Duration, sleep};

    info!("Using polling fallback (every 10 seconds)");
    loop {
        sleep(Duration::from_secs(10)).await;
        send_update();
    }
}
