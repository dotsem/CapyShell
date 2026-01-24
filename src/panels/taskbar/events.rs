//! Taskbar event definitions and broadcast event bus.
//!
//! Uses tokio::sync::broadcast so ALL taskbars receive every event.
//! Per-monitor events can be filtered by the receiver using the monitor tag.

use crate::event_bus::CHANNEL_CAPACITY;
use crate::services::battery::BatteryStatus;
use crate::services::bluetooth::BluetoothStatus;
use crate::services::media::MprisData;
use crate::services::network::NetworkStatus;
use crate::services::system_info::SystemStatus;
use crate::services::volume::VolumeStatus;
use crate::services::wm::{ActiveWindowInfo, WorkspacesStatus};
use std::sync::OnceLock;
use tokio::sync::broadcast::{self, Receiver, Sender};

/// All possible taskbar events from background services.
#[derive(Clone, Debug)]
pub enum TaskbarEvent {
    // Global events (same for all monitors)
    Battery(BatteryStatus),
    Volume(VolumeStatus),
    Network(NetworkStatus),
    Bluetooth(BluetoothStatus),
    // Per-monitor events (filtered by receiver)
    Workspaces(WorkspacesStatus),
    Mpris(Box<crate::services::media::MprisData>), // Boxed to keep enum size small
    ActiveWindow(ActiveWindowInfo),
    SystemStatus(SystemStatus),
}

impl TaskbarEvent {
    /// Get variant index for deduplication.
    #[inline]
    pub fn variant_index(&self) -> usize {
        match self {
            TaskbarEvent::Battery(_) => 0,
            TaskbarEvent::Volume(_) => 1,
            TaskbarEvent::Network(_) => 2,
            TaskbarEvent::Bluetooth(_) => 3,
            TaskbarEvent::Workspaces(_) => 4,
            TaskbarEvent::Mpris(_) => 5,
            TaskbarEvent::ActiveWindow(_) => 6,
            TaskbarEvent::SystemStatus(_) => 7,
        }
    }
}

// Static broadcast sender - subscribers get their own receiver via subscribe()
static TASKBAR_SENDER: OnceLock<Sender<TaskbarEvent>> = OnceLock::new();

fn get_sender() -> &'static Sender<TaskbarEvent> {
    TASKBAR_SENDER.get_or_init(|| {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        tx
    })
}

/// Send an event to all taskbars. Non-blocking.
/// If no receivers, the event is dropped (expected during startup).
#[inline]
pub fn send(event: TaskbarEvent) {
    let _ = get_sender().send(event);
}

/// Send battery data to all taskbars.
#[inline]
pub fn send_battery(data: BatteryStatus) {
    send(TaskbarEvent::Battery(data));
}

/// Send volume data to all taskbars.
#[inline]
pub fn send_volume(data: VolumeStatus) {
    send(TaskbarEvent::Volume(data));
}

/// Send network data to all taskbars.
#[inline]
pub fn send_network(data: NetworkStatus) {
    send(TaskbarEvent::Network(data));
}

/// Send bluetooth data to all taskbars.
#[inline]
pub fn send_bluetooth(data: BluetoothStatus) {
    send(TaskbarEvent::Bluetooth(data));
}

/// Send workspaces data to all taskbars (filtered by monitor name).
#[inline]
pub fn send_workspaces(data: WorkspacesStatus) {
    send(TaskbarEvent::Workspaces(data));
}

/// Send mpris data to all taskbars.
#[inline]
pub fn send_mpris(data: MprisData) {
    send(TaskbarEvent::Mpris(Box::new(data)));
}

/// Send active window data to all taskbars.
#[inline]
pub fn send_active_window(data: ActiveWindowInfo) {
    send(TaskbarEvent::ActiveWindow(data));
}

/// Subscribe to the event bus. Each taskbar gets its own receiver.
/// Returns a new receiver that will receive all future events.
pub fn subscribe() -> Receiver<TaskbarEvent> {
    get_sender().subscribe()
}

/// Drain all pending events from a receiver, keeping only the latest per variant.
/// Workspaces events are NOT deduplicated since they are per-monitor.
/// Handles RecvError::Lagged by continuing to drain.
#[inline]
pub fn drain_latest(rx: &mut Receiver<TaskbarEvent>) -> Vec<TaskbarEvent> {
    let mut events = Vec::with_capacity(8);

    loop {
        match rx.try_recv() {
            Ok(event) => events.push(event),
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue, // Skip old, keep draining
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }

    if events.len() <= 1 {
        return events;
    }

    // Deduplicate: keep only the latest of each variant
    // EXCEPT Workspaces events (variant 4) which are per-monitor
    let mut seen = [false; 8]; // Support up to 8 event types
    let mut result = Vec::with_capacity(events.len());

    for event in events.into_iter().rev() {
        let idx = event.variant_index();

        // Keep ALL Workspaces events (they're per-monitor, each one matters)
        if idx == 4 {
            result.push(event);
        } else if !seen[idx] {
            seen[idx] = true;
            result.push(event);
        }
    }

    result.reverse();
    result
}
