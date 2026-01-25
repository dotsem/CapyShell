//! MPRIS UI Adapter
//! Bridges MPRIS service data to Slint UI and handles control callbacks.
//! Includes position interpolation for smooth progress bar updates.

use crate::panels::taskbar::{MediaData, Taskbar};
use crate::services::media::{MprisData as ServiceMprisData, send_command};
use capy_mpris::PlayerCommand;
use log::{debug, warn};
use slint::{ComponentHandle, Image, Timer, TimerMode};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

// Thread-local state shared between update_ui and the interpolation timer
thread_local! {
    // Image cache
    static CACHED_ART_PATH: RefCell<String> = RefCell::new(String::new());
    static CACHED_ART: RefCell<Image> = RefCell::new(Image::default());
    static CACHED_BLUR_PATH: RefCell<String> = RefCell::new(String::new());
    static CACHED_BLUR: RefCell<Image> = RefCell::new(Image::default());

    // Server-sent state (written by update_ui, read by timer)
    static SERVER_STATE: RefCell<ServerState> = RefCell::new(ServerState::default());
}

/// State received from the MPRIS service (not interpolated)
#[derive(Default, Clone)]
struct ServerState {
    position_secs: f32,
    is_playing: bool,
    length_secs: f32,
    title_hash: u64,
    updated_at: Option<Instant>,
}

/// Interpolation state (managed by timer)
struct InterpolationState {
    base_position: f32,
    base_time: Instant,
    last_server_update: Option<Instant>, // Track when we last synced with server
}

impl Default for InterpolationState {
    fn default() -> Self {
        Self {
            base_position: 0.0,
            base_time: Instant::now(),
            last_server_update: None,
        }
    }
}

fn hash_string(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Called by event loop when new MPRIS data arrives from service
pub fn update_ui(ui: &Taskbar, data: &ServiceMprisData) {
    // Debug: trace is_playing value
    debug!(
        "update_ui: is_playing={}, title='{}'",
        data.is_playing, data.title
    );

    let text_color = slint::Color::from_rgb_u8(255, 255, 255);

    // Clear in-memory cache on track change to show placeholder
    if data.is_track_change {
        CACHED_ART_PATH.with(|p| p.borrow_mut().clear());
        CACHED_ART.with(|i| *i.borrow_mut() = Image::default());
        CACHED_BLUR_PATH.with(|p| p.borrow_mut().clear());
        CACHED_BLUR.with(|i| *i.borrow_mut() = Image::default());
    }

    // Handle images with caching
    let album_art = get_or_keep_cached_image(
        data.album_art_path.as_str(),
        &CACHED_ART_PATH,
        &CACHED_ART,
        data.has_media,
    );
    let blurred_art = get_or_keep_cached_image(
        data.blurred_art_path.as_str(),
        &CACHED_BLUR_PATH,
        &CACHED_BLUR,
        data.has_media,
    );

    // Update server state (for timer to read)
    let title_hash = hash_string(data.title.as_str());
    SERVER_STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.position_secs = data.position_secs;
        state.is_playing = data.is_playing;
        state.length_secs = data.length_secs;
        state.title_hash = title_hash;
        state.updated_at = Some(Instant::now());
    });

    let media_data = MediaData {
        title: data.title.clone(),
        artist: data.artist.clone(),
        album: data.album.clone(),
        album_art,
        blurred_art,
        length_secs: data.length_secs,
        position_secs: data.position_secs,
        is_playing: data.is_playing,
        has_media: data.has_media,
        text_color,
    };

    ui.set_media_data(media_data);
}

fn get_or_keep_cached_image(
    path: &str,
    cached_path: &'static std::thread::LocalKey<RefCell<String>>,
    cached_img: &'static std::thread::LocalKey<RefCell<Image>>,
    has_media: bool,
) -> Image {
    if !has_media {
        cached_path.with(|p| p.borrow_mut().clear());
        cached_img.with(|i| *i.borrow_mut() = Image::default());
        return Image::default();
    }

    if path.is_empty() {
        return cached_img.with(|i| i.borrow().clone());
    }

    let is_cached = cached_path.with(|p| p.borrow().as_str() == path);
    if is_cached {
        return cached_img.with(|i| i.borrow().clone());
    }

    match Image::load_from_path(std::path::Path::new(path)) {
        Ok(img) => {
            cached_path.with(|p| *p.borrow_mut() = path.to_string());
            cached_img.with(|i| *i.borrow_mut() = img.clone());
            img
        }
        Err(e) => {
            warn!("Failed to load image {}: {}", path, e);
            cached_img.with(|i| i.borrow().clone())
        }
    }
}

/// Attach callbacks and start timers
pub fn attach_callbacks(ui: &Taskbar) {
    // Playback controls - now using capy-mpris commands
    ui.on_media_play_pause(|| {
        send_command(PlayerCommand::PlayPause);
    });

    ui.on_media_next(|| {
        send_command(PlayerCommand::Next);
    });

    ui.on_media_prev(|| {
        send_command(PlayerCommand::Previous);
    });

    ui.on_media_seek(|percent| {
        // Get current length from server state and calculate position
        let length_secs = SERVER_STATE.with(|s| s.borrow().length_secs);
        let position_us = (length_secs * percent * 1_000_000.0) as i64;
        send_command(PlayerCommand::SetPosition(position_us));
    });

    // Interpolation state (local to timer)
    let interp_state = Rc::new(RefCell::new(InterpolationState::default()));

    // Track last rendered position to avoid redundant updates
    let last_rendered_position = Rc::new(RefCell::new(0.0f32));

    // Position interpolation timer (100ms)
    let ui_weak = ui.as_weak();
    let interp_clone = interp_state.clone();
    let last_pos_clone = last_rendered_position.clone();

    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
        if let Some(ui) = ui_weak.upgrade() {
            let current_data = ui.get_media_data();

            if !current_data.has_media {
                return;
            }

            // Read server state
            let server = SERVER_STATE.with(|s| s.borrow().clone());

            // ALWAYS sync with server when it has a new update
            // This ensures server position is always our source of truth
            let server_updated = server.updated_at;
            let needs_resync = {
                let interp = interp_clone.borrow();
                // Sync if server has updated since our last sync
                server_updated != interp.last_server_update
            };

            if needs_resync {
                let mut interp = interp_clone.borrow_mut();
                interp.base_position = server.position_secs;
                interp.base_time = Instant::now();
                interp.last_server_update = server_updated;
            }

            // Calculate interpolated position
            let new_position = {
                let interp = interp_clone.borrow();
                if server.is_playing {
                    // When playing, interpolate from base
                    let elapsed = interp.base_time.elapsed().as_secs_f32();
                    (interp.base_position + elapsed)
                        .min(server.length_secs)
                        .max(0.0)
                } else {
                    // When paused, always use the server's position directly
                    server.position_secs
                }
            };

            // Only update UI if position actually changed
            let last_pos = *last_pos_clone.borrow();
            let position_changed = (new_position - last_pos).abs() > 0.01; // ~10ms threshold

            if position_changed {
                *last_pos_clone.borrow_mut() = new_position;

                let mut updated_data = current_data;
                updated_data.position_secs = new_position;
                ui.set_media_data(updated_data);
            }
        }
    });
    std::mem::forget(timer);
}
