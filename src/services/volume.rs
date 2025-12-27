//! Volume monitoring service using PulseAudio/PipeWire.
//!
//! Spawns a single thread that monitors all sinks and broadcasts
//! volume changes to all panels via the event bus.
//!
//! Uses libpulse-binding which works with both PulseAudio and PipeWire
//! (via pipewire-pulse compatibility layer).

use crate::panels::taskbar::events;
use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::introspect::SinkInfo;
use libpulse_binding::context::subscribe::{Facility, InterestMaskSet, Operation};
use libpulse_binding::context::{Context, FlagSet, State as ContextState};
use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
use libpulse_binding::volume::Volume;
use log::{debug, error, info, warn};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

/// Volume status for a sink (audio output device).
#[derive(Clone, Debug)]
pub struct VolumeStatus {
    /// Volume percentage (0-100+, can exceed 100 for amplified audio)
    pub volume_percent: i32,
    /// Whether the sink is muted
    pub muted: bool,
    /// Sink name (e.g., "alsa_output.pci-0000_00_1f.3.analog-stereo")
    pub sink_name: String,
    /// Human-readable description
    pub description: String,
}

/// Start the volume monitoring background thread.
pub fn start_monitor() {
    info!("Starting volume monitor...");

    thread::spawn(move || {
        if let Err(e) = run_mainloop() {
            error!("Volume monitor failed: {}", e);
        }
    });
}

/// Get current default sink volume (blocking call for initial state).
pub fn get_default_volume() -> Option<VolumeStatus> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let Some(mut ml) = Mainloop::new() else {
            return;
        };

        let Some(mut ctx) = Context::new(&ml, "CapyShell Volume Query") else {
            return;
        };

        if ctx.connect(None, FlagSet::NOFLAGS, None).is_err() {
            return;
        }

        // Wait for ready
        loop {
            match ml.iterate(true) {
                IterateResult::Success(_) => {}
                _ => return,
            }
            match ctx.get_state() {
                ContextState::Ready => break,
                ContextState::Failed | ContextState::Terminated => return,
                _ => {}
            }
        }

        // Query default sink
        let tx_cell = Rc::new(RefCell::new(Some(tx)));
        let tx_clone = Rc::clone(&tx_cell);

        ctx.introspect()
            .get_sink_info_by_name("@DEFAULT_SINK@", move |result| {
                if let ListResult::Item(info) = result {
                    let status = sink_info_to_status(info);
                    if let Some(tx) = tx_clone.borrow_mut().take() {
                        let _ = tx.send(status);
                    }
                }
            });

        // Process until we get the result
        for _ in 0..20 {
            if tx_cell.borrow().is_none() {
                break;
            }
            if let IterateResult::Err(_) = ml.iterate(true) {
                break;
            }
        }
    });

    rx.recv_timeout(std::time::Duration::from_secs(2)).ok()
}

// === Internal implementation ===

fn run_mainloop() -> Result<(), Box<dyn std::error::Error>> {
    let mut ml = Mainloop::new().ok_or("Failed to create mainloop")?;
    let mut ctx =
        Context::new(&ml, "CapyShell Volume Monitor").ok_or("Failed to create context")?;

    ctx.connect(None, FlagSet::NOFLAGS, None)
        .map_err(|e| format!("Failed to connect: {:?}", e))?;

    // Wait for context to be ready
    loop {
        match ml.iterate(true) {
            IterateResult::Quit(_) => return Err("Mainloop quit during connect".into()),
            IterateResult::Err(e) => return Err(format!("Mainloop error: {:?}", e).into()),
            IterateResult::Success(_) => {}
        }

        match ctx.get_state() {
            ContextState::Ready => break,
            ContextState::Failed | ContextState::Terminated => {
                return Err("Context connection failed".into());
            }
            _ => continue,
        }
    }

    info!("Connected to PulseAudio/PipeWire");

    // Subscribe to sink and server events
    ctx.subscribe(InterestMaskSet::SINK | InterestMaskSet::SERVER, |success| {
        if !success {
            warn!("Failed to subscribe to PulseAudio events");
        }
    });

    // Flag to request sink info query
    let needs_query = Rc::new(RefCell::new(true)); // Start with initial query
    let needs_query_cb = Rc::clone(&needs_query);

    ctx.set_subscribe_callback(Some(Box::new(move |facility, operation, _index| {
        match (facility, operation) {
            (Some(Facility::Sink), Some(Operation::Changed | Operation::New))
            | (Some(Facility::Server), _) => {
                *needs_query_cb.borrow_mut() = true;
            }
            _ => {}
        }
    })));

    info!("Volume monitor listening for changes...");

    // Main event loop
    loop {
        match ml.iterate(true) {
            IterateResult::Quit(_) => break,
            IterateResult::Err(e) => {
                error!("Mainloop error: {:?}", e);
                break;
            }
            IterateResult::Success(_) => {}
        }

        // Check if we need to query (outside of callbacks)
        if *needs_query.borrow() {
            *needs_query.borrow_mut() = false;

            ctx.introspect()
                .get_sink_info_by_name("@DEFAULT_SINK@", |result| {
                    if let ListResult::Item(info) = result {
                        let status = sink_info_to_status(info);
                        debug!(
                            "Volume update: {}% (muted: {})",
                            status.volume_percent, status.muted
                        );
                        send_update(status);
                    }
                });
        }
    }

    Ok(())
}

fn sink_info_to_status(info: &SinkInfo) -> VolumeStatus {
    let volume = info.volume.avg();
    let volume_percent = volume_to_percent(volume);
    let muted = info.mute;
    let sink_name = info
        .name
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let description = info
        .description
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_default();

    VolumeStatus {
        volume_percent,
        muted,
        sink_name,
        description,
    }
}

fn volume_to_percent(volume: Volume) -> i32 {
    let normal = Volume::NORMAL.0 as f64;
    let current = volume.0 as f64;
    ((current / normal) * 100.0).round() as i32
}

/// Send volume update to event bus.
fn send_update(status: VolumeStatus) {
    events::send_volume(status);
}
