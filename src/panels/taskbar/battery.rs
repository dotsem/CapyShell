use crate::panels::taskbar::taskbar::{BatterState, BatteryData, Taskbar};
use battery::{Battery, Manager, State};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::sync::OnceLock;
use std::thread;

/// Global channel for battery updates (thread-safe, initialized once)
static BATTERY_CHANNEL: OnceLock<(Sender<BatteryData>, Receiver<BatteryData>)> = OnceLock::new();

fn get_channel() -> &'static (Sender<BatteryData>, Receiver<BatteryData>) {
    BATTERY_CHANNEL.get_or_init(|| unbounded())
}

/// Get the receiver for polling from the UI thread
pub fn get_battery_receiver() -> Receiver<BatteryData> {
    get_channel().1.clone()
}

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

/// Update the UI with current battery info (called from event loop)
pub fn update_battery_info(ui: &Taskbar) {
    let battery_data = get_battery_data();
    ui.set_battery_state(battery_data);
}

/// Start the battery monitoring background thread.
/// Sends updates to a channel that should be polled by a Slint Timer.
pub fn start_battery_monitor() {
    println!("Starting battery monitor...");
    let tx = get_channel().0.clone();

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            if let Err(e) = dbus_worker(&tx).await {
                eprintln!("D-Bus worker failed: {}. Falling back to polling.", e);
                polling_worker(&tx).await;
            }
        });
    });
}

fn send_battery_update(tx: &Sender<BatteryData>) {
    let data = get_battery_data();
    println!("Battery update: {}%, {:?}", data.percentage, data.state);
    let _ = tx.send(data);
}

async fn dbus_worker(
    tx: &Sender<BatteryData>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            send_battery_update(tx);
        }
    }

    eprintln!("D-Bus signal stream ended");
    Ok(())
}

async fn polling_worker(tx: &Sender<BatteryData>) {
    use tokio::time::{Duration, sleep};

    println!("Using polling fallback (every 30 seconds)");
    loop {
        sleep(Duration::from_secs(30)).await;
        send_battery_update(tx);
    }
}
