use crate::panels::taskbar::taskbar::{BatterState, BatteryData, Taskbar};
use std::thread;

pub fn get_battery_data() -> BatteryData {
    use battery::{Manager, State};

    match Manager::new() {
        Ok(manager) => {
            match manager.batteries() {
                Ok(mut batteries) => {
                    if let Some(Ok(mut battery)) = batteries.next() {
                        if let Err(e) = manager.refresh(&mut battery) {
                            eprintln!("Failed to refresh battery: {}", e);
                            return BatteryData {
                                percentage: 0,
                                state: BatterState::Unknown,
                                icon: "󰂑".into(),
                            };
                        }

                        let percentage = (battery.state_of_charge().value * 100.0) as i32;
                        let battery_state = battery.state();

                        println!("Battery: {}%, Raw state: {:?}", percentage, battery_state);

                        let state = match battery_state {
                            State::Charging => BatterState::Charging,
                            State::Full => {
                                // Check if still plugged in (some systems report Full while charging)
                                BatterState::Full
                            }
                            State::Discharging => match percentage {
                                0..=4 => BatterState::Critical,
                                5..=14 => BatterState::Low,
                                15..=29 => BatterState::S1,
                                30..=39 => BatterState::S2,
                                40..=49 => BatterState::S3,
                                50..=59 => BatterState::S4,
                                60..=79 => BatterState::S5,
                                80..=94 => BatterState::S6,
                                _ => BatterState::Full,
                            },
                            State::Empty => BatterState::Critical,
                            State::Unknown => {
                                // When state is unknown, infer from percentage and other info
                                println!("Battery state is Unknown, checking energy rate...");
                                let energy_rate = battery.energy_rate().value;
                                println!("Energy rate: {}", energy_rate);

                                if energy_rate > 0.0 {
                                    // Positive energy rate usually means charging
                                    BatterState::Charging
                                } else if percentage >= 95 {
                                    BatterState::Full
                                } else {
                                    match percentage {
                                        0..=4 => BatterState::Critical,
                                        5..=14 => BatterState::Low,
                                        15..=29 => BatterState::S1,
                                        30..=39 => BatterState::S2,
                                        40..=49 => BatterState::S3,
                                        50..=59 => BatterState::S4,
                                        60..=79 => BatterState::S5,
                                        80..=94 => BatterState::S6,
                                        _ => BatterState::S6,
                                    }
                                }
                            }
                            _ => {
                                println!("Unhandled battery state: {:?}", battery_state);
                                BatterState::Unknown
                            }
                        };
                        println!("Percentage: {}, State: {}", percentage, battery.state());

                        let icon = match state {
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
                        };

                        return BatteryData {
                            percentage,
                            state,
                            icon: icon.into(),
                        };
                    }
                }
                Err(e) => eprintln!("Failed to get batteries: {}", e),
            }
        }
        Err(e) => eprintln!("Failed to create battery manager: {}", e),
    }

    BatteryData {
        percentage: 0,
        state: BatterState::Unknown,
        icon: "󰂑".into(),
    }
}

pub fn update_battery_info(ui: &Taskbar) {
    let battery_data = get_battery_data();
    ui.set_battery_state(battery_data);
}

pub fn start_battery_monitor(ui_weak: slint::Weak<Taskbar>) {
    // Spawn a tokio runtime in a separate thread for async D-Bus operations
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            // Initial update
            let battery_data = get_battery_data();
            let ui_weak_clone = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_clone.upgrade() {
                    ui.set_battery_state(battery_data);
                }
            });

            // Try to connect to UPower D-Bus service
            match setup_upower_monitoring(ui_weak.clone()).await {
                Ok(_) => {
                    println!("Successfully set up UPower D-Bus monitoring");
                }
                Err(e) => {
                    eprintln!(
                        "Failed to set up UPower monitoring: {}. Falling back to polling.",
                        e
                    );
                    // Fall back to polling if D-Bus doesn't work
                    polling_fallback(ui_weak).await;
                }
            }
        });
    });
}

async fn setup_upower_monitoring(
    ui_weak: slint::Weak<Taskbar>,
) -> Result<(), Box<dyn std::error::Error>> {
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

    // Listen for property changes
    let mut properties_changed = proxy.receive_properties_changed().await?;

    println!("Listening for UPower D-Bus signals...");

    while let Some(signal) = properties_changed.next().await {
        match signal.args() {
            Ok(_args) => {
                println!("Battery property changed, updating...");
                let battery_data = get_battery_data();
                let ui_weak_clone = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_clone.upgrade() {
                        ui.set_battery_state(battery_data);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error receiving signal: {}", e);
            }
        }
    }

    eprintln!("D-Bus signal stream ended");
    Ok(())
}

async fn polling_fallback(ui_weak: slint::Weak<Taskbar>) {
    use tokio::time::{Duration, sleep};

    println!("Using polling fallback (every 10 seconds)");
    loop {
        sleep(Duration::from_secs(10)).await;

        let battery_data = get_battery_data();
        let ui_weak_clone = ui_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak_clone.upgrade() {
                ui.set_battery_state(battery_data);
            }
        });
    }
}
