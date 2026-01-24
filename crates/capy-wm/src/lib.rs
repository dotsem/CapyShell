//! capy-wm: Window manager abstraction for tiling window managers
//!
//! Provides a unified interface for interacting with tiling window managers.
//! Currently supports Hyprland with architecture ready for Sway, Niri, etc.

pub mod types;
pub mod window_backend;

#[cfg(feature = "hyprland")]
pub mod hyprland;

pub use types::*;
pub use window_backend::*;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

/// Icon resolver callback type.
/// The WM backend uses this to resolve app class names to icon paths.
pub type IconResolver = Box<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>;

/// Event callback type.
/// Called when WM events occur (workspace changes, active window changes, etc.).
pub type EventCallback = Box<dyn Fn(WmEvent) + Send + Sync>;

// Global callbacks shared across the backend
static ICON_RESOLVER: OnceLock<Arc<RwLock<Option<IconResolver>>>> = OnceLock::new();
static EVENT_CALLBACK: OnceLock<Arc<RwLock<Option<EventCallback>>>> = OnceLock::new();

// Global state cache
static STATE: OnceLock<WmState> = OnceLock::new();

/// Global window manager state cache.
pub struct WmState {
    /// Currently active window info.
    pub active_window: RwLock<ActiveWindowInfo>,
    /// Workspace status keyed by monitor name.
    pub workspaces: RwLock<HashMap<String, WorkspacesStatus>>,
}

/// Returns the active window info.
pub fn get_active_window() -> ActiveWindowInfo {
    let state = get_state();
    match state.active_window.read() {
        Ok(w) => w.clone(),
        Err(err) => {
            eprintln!(
                "capy-wm: failed to acquire read lock on active_window in get_active_window: {:?}",
                err
            );
            ActiveWindowInfo::default()
        }
    }
}
pub fn get_workspaces_status(monitor_name: &str) -> WorkspacesStatus {
    let state = get_state();
    match state.workspaces.read() {
        Ok(map) => map
            .get(monitor_name)
            .cloned()
            .unwrap_or_else(|| WorkspacesStatus {
                monitor_name: monitor_name.to_string(),
                workspaces: Vec::new(),
            }),
        Err(err) => {
            eprintln!(
                "capy-wm: failed to acquire read lock on workspaces in get_workspaces_status for monitor '{}': {:?}",
                monitor_name, err
            );
            WorkspacesStatus {
                monitor_name: monitor_name.to_string(),
                workspaces: Vec::new(),
            }
        }
    }
}
/// Get the active monitor name.
pub fn get_active_monitor() -> String {
    let state = get_state();
    match state.active_window.read() {
        Ok(w) => w.focused_monitor.clone(),
        Err(err) => {
            eprintln!(
                "capy-wm: failed to acquire read lock on active_window in get_active_monitor: {:?}",
                err
            );
            String::new()
        }
    }
}

impl WmState {
    fn new() -> Self {
        Self {
            active_window: RwLock::new(ActiveWindowInfo::default()),
            workspaces: RwLock::new(HashMap::new()),
        }
    }
}

/// Get the global state instance.
pub fn get_state() -> &'static WmState {
    STATE.get_or_init(WmState::new)
}

fn get_icon_resolver_store() -> Arc<RwLock<Option<IconResolver>>> {
    ICON_RESOLVER
        .get_or_init(|| Arc::new(RwLock::new(None)))
        .clone()
}

fn get_event_callback_store() -> Arc<RwLock<Option<EventCallback>>> {
    EVENT_CALLBACK
        .get_or_init(|| Arc::new(RwLock::new(None)))
        .clone()
}

/// Set the icon resolver callback.
pub fn set_icon_resolver(resolver: IconResolver) {
    match get_icon_resolver_store().write() {
        Ok(mut guard) => {
            *guard = Some(resolver);
        }
        Err(err) => {
            eprintln!(
                "capy-wm: failed to acquire write lock on icon resolver: {:?}",
                err
            );
        }
    }
}

/// Set the event callback.
pub fn set_event_callback<F>(callback: F)
where
    F: Fn(WmEvent) + Send + Sync + 'static,
{
    match get_event_callback_store().write() {
        Ok(mut guard) => {
            *guard = Some(Box::new(callback));
        }
        Err(err) => {
            eprintln!(
                "capy-wm: failed to acquire write lock on event callback: {:?}",
                err
            );
        }
    }
}

/// Resolve an icon path using the configured resolver.
pub fn resolve_icon(class: &str) -> Option<PathBuf> {
    match get_icon_resolver_store().read() {
        Ok(guard) => guard.as_ref().and_then(|r| r(class)),
        Err(err) => {
            eprintln!(
                "capy-wm: failed to acquire read lock on icon resolver in resolve_icon: {:?}",
                err
            );
            None
        }
    }
}

/// Updates internal cache and sends a WM event to the configured callback.
pub fn send_event(event: WmEvent) {
    let state = get_state();
    let mut update_successful = true;

    match &event {
        WmEvent::WorkspacesChanged(status) => match state.workspaces.write() {
            Ok(mut guard) => {
                guard.insert(status.monitor_name.clone(), status.clone());
            }
            Err(err) => {
                eprintln!(
                    "capy-wm: failed to acquire write lock on workspaces in send_event: {:?}",
                    err
                );
                update_successful = false;
            }
        },
        WmEvent::ActiveWindowChanged(info) => match state.active_window.write() {
            Ok(mut guard) => {
                *guard = info.clone();
            }
            Err(err) => {
                eprintln!(
                    "capy-wm: failed to acquire write lock on active_window in send_event: {:?}",
                    err
                );
                update_successful = false;
            }
        },
        _ => {}
    }

    if update_successful {
        match get_event_callback_store().read() {
            Ok(guard) => {
                if let Some(ref callback) = *guard {
                    callback(event);
                }
            }
            Err(err) => {
                eprintln!(
                    "capy-wm: failed to acquire read lock on event callback store in send_event: {:?}",
                    err
                );
            }
        }
    }
}
