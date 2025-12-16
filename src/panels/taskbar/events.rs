//! Taskbar event definitions and event bus.
//!
//! All background services send events here, and a single Timer
//! polls and applies them to the UI.

use crate::event_bus::CHANNEL_CAPACITY;
use crate::panels::taskbar::battery::BatteryStatus;
use crossbeam_channel::{Receiver, Sender, bounded};
use std::sync::OnceLock;

/// All possible taskbar events from background services.
#[derive(Clone, Debug)]
pub enum TaskbarEvent {
    Battery(BatteryStatus),
    // Future events:
    // Music(MusicData),
    // Systray(SystrayData),
    // Network(NetworkData),
    // Volume(VolumeData),
}

impl TaskbarEvent {
    /// Get variant index for deduplication.
    #[inline]
    pub fn variant_index(&self) -> usize {
        match self {
            TaskbarEvent::Battery(_) => 0,
            // TaskbarEvent::Music(_) => 1,
            // TaskbarEvent::Systray(_) => 2,
            // etc.
        }
    }
}

// Static channel for taskbar events
static TASKBAR_CHANNEL: OnceLock<(Sender<TaskbarEvent>, Receiver<TaskbarEvent>)> = OnceLock::new();

fn get_channel() -> &'static (Sender<TaskbarEvent>, Receiver<TaskbarEvent>) {
    TASKBAR_CHANNEL.get_or_init(|| bounded(CHANNEL_CAPACITY))
}

/// Send an event to the taskbar. Non-blocking.
/// If buffer is full, drops the event (we always want latest data anyway).
#[inline]
pub fn send(event: TaskbarEvent) {
    let tx = &get_channel().0;
    let _ = tx.try_send(event);
}

/// Send battery data to the taskbar.
#[inline]
pub fn send_battery(data: BatteryStatus) {
    send(TaskbarEvent::Battery(data));
}

/// Get the receiver for polling from the UI thread.
pub fn receiver() -> Receiver<TaskbarEvent> {
    get_channel().1.clone()
}

/// Drain all pending events, keeping only the latest per variant.
/// This is the most efficient way to handle bursts of events.
#[inline]
pub fn drain_latest(rx: &Receiver<TaskbarEvent>) -> Vec<TaskbarEvent> {
    let mut events = Vec::with_capacity(CHANNEL_CAPACITY);
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    if events.len() <= 1 {
        return events;
    }

    // Deduplicate: keep only the latest of each variant
    let mut seen = [false; 8]; // Support up to 8 event types
    let mut result = Vec::with_capacity(events.len());

    for event in events.into_iter().rev() {
        let idx = event.variant_index();
        if !seen[idx] {
            seen[idx] = true;
            result.push(event);
        }
    }

    result.reverse();
    result
}
