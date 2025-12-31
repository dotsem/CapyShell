//! MPRIS UI Adapter
//! Bridges MPRIS service data to Slint UI and handles control callbacks.
//! Includes position interpolation for smooth progress bar updates.

use crate::panels::taskbar::taskbar::{MediaData, Taskbar};
use crate::services::media::MprisData as ServiceMprisData;
use log::{error, info, warn};
use mpris::PlayerFinder;
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
    last_title_hash: u64,
    last_is_playing: bool,
}

impl Default for InterpolationState {
    fn default() -> Self {
        Self {
            base_position: 0.0,
            base_time: Instant::now(),
            last_title_hash: 0,
            last_is_playing: false,
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
    // Playback controls
    ui.on_media_play_pause(|| {
        media_command(|p| {
            let _ = p.play_pause();
        })
    });
    ui.on_media_next(|| {
        media_command(|p| {
            let _ = p.next();
        })
    });
    ui.on_media_prev(|| {
        media_command(|p| {
            let _ = p.previous();
        })
    });
    ui.on_media_seek(|percent| {
        media_command(move |player| {
            if let Ok(metadata) = player.get_metadata() {
                if let Some(length) = metadata.length() {
                    let position = length.as_secs_f32() * percent;
                    if let Some(track_id) = metadata.track_id() {
                        let _ = player.set_position(track_id, &Duration::from_secs_f32(position));
                    }
                }
            }
        });
    });

    // Interpolation state (local to timer)
    let interp_state = Rc::new(RefCell::new(InterpolationState::default()));

    // Position interpolation timer (100ms)
    let ui_weak = ui.as_weak();
    let interp_clone = interp_state.clone();

    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
        if let Some(ui) = ui_weak.upgrade() {
            let current_data = ui.get_media_data();

            if !current_data.has_media {
                return;
            }

            // Read server state
            let server = SERVER_STATE.with(|s| s.borrow().clone());

            // Check if we need to resync interpolation
            let needs_resync = {
                let interp = interp_clone.borrow();
                // Track changed
                server.title_hash != interp.last_title_hash
                // Playback state changed
                || server.is_playing != interp.last_is_playing
                // Server position differs significantly from our base
                // (this catches seeks and external position changes)
                || (server.position_secs - interp.base_position).abs() > 1.0
            };

            if needs_resync {
                let mut interp = interp_clone.borrow_mut();
                interp.base_position = server.position_secs;
                interp.base_time = Instant::now();
                interp.last_title_hash = server.title_hash;
                interp.last_is_playing = server.is_playing;
            }

            // Calculate interpolated position
            let new_position = {
                let interp = interp_clone.borrow();
                if server.is_playing {
                    let elapsed = interp.base_time.elapsed().as_secs_f32();
                    (interp.base_position + elapsed)
                        .min(server.length_secs)
                        .max(0.0)
                } else {
                    interp.base_position
                }
            };

            // Update UI
            let mut updated_data = current_data;
            updated_data.position_secs = new_position;
            ui.set_media_data(updated_data);
        }
    });
    std::mem::forget(timer);

    // Initial state query with album art loading
    let ui_weak_init = ui.as_weak();
    let init_timer = Timer::default();
    init_timer.start(
        TimerMode::SingleShot,
        Duration::from_millis(300),
        move || {
            query_and_load_initial_state(&ui_weak_init);
        },
    );
    std::mem::forget(init_timer);
}

/// Query initial state including album art processing
fn query_and_load_initial_state(ui_weak: &slint::Weak<Taskbar>) {
    info!("Querying initial MPRIS state with album art...");

    let finder = match PlayerFinder::new() {
        Ok(f) => f,
        Err(e) => {
            warn!("No PlayerFinder available: {}", e);
            return;
        }
    };

    let player = match finder.find_active() {
        Ok(p) => p,
        Err(_) => {
            info!("No active player found");
            return;
        }
    };

    let metadata = match player.get_metadata() {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to get metadata: {}", e);
            return;
        }
    };

    let title = metadata.title().unwrap_or("Unknown").to_string();
    let artists = metadata.artists().map(|a| a.join(", ")).unwrap_or_default();
    let art_url = metadata.art_url().unwrap_or("");
    let length = metadata.length().unwrap_or(Duration::ZERO);
    let is_playing = player
        .get_playback_status()
        .map(|s| s == mpris::PlaybackStatus::Playing)
        .unwrap_or(false);
    let position = player.get_position().unwrap_or(Duration::ZERO);

    // Process album art if available
    let (album_art, blurred_art) = if !art_url.is_empty() {
        load_album_art_sync(art_url)
    } else {
        (Image::default(), Image::default())
    };

    // Update server state
    let title_hash = hash_string(&title);
    SERVER_STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.position_secs = position.as_secs_f32();
        state.is_playing = is_playing;
        state.length_secs = length.as_secs_f32();
        state.title_hash = title_hash;
        state.updated_at = Some(Instant::now());
    });

    if let Some(ui) = ui_weak.upgrade() {
        let text_color = slint::Color::from_rgb_u8(255, 255, 255);
        let media_data = MediaData {
            title: title.into(),
            artist: artists.into(),
            album_art,
            blurred_art,
            length_secs: length.as_secs_f32(),
            position_secs: position.as_secs_f32(),
            is_playing,
            has_media: true,
            text_color,
        };
        ui.set_media_data(media_data);
        info!("Initial state with art loaded");
    }
}

/// Load album art synchronously (for initial load)
fn load_album_art_sync(url: &str) -> (Image, Image) {
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io::Read;

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let cache_dir = std::path::PathBuf::from(home).join(".cache/CapyShell/thumbs");
    let _ = fs::create_dir_all(&cache_dir);

    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hex::encode(hasher.finalize());

    let original_path = cache_dir.join(format!("{}.png", hash));
    let blur_path = cache_dir.join(format!("{}_blur.png", hash));

    // Try loading cached images first
    if original_path.exists() && blur_path.exists() {
        let art = Image::load_from_path(&original_path).ok();
        let blur = Image::load_from_path(&blur_path).ok();
        if let (Some(a), Some(b)) = (art, blur) {
            // Cache in thread-local
            CACHED_ART_PATH.with(|p| *p.borrow_mut() = original_path.to_string_lossy().to_string());
            CACHED_ART.with(|i| *i.borrow_mut() = a.clone());
            CACHED_BLUR_PATH.with(|p| *p.borrow_mut() = blur_path.to_string_lossy().to_string());
            CACHED_BLUR.with(|i| *i.borrow_mut() = b.clone());
            return (a, b);
        }
    }

    // Need to download and process
    let img_data = if url.starts_with("file://") {
        let path = url.strip_prefix("file://").unwrap();
        match fs::read(path) {
            Ok(d) => d,
            Err(_) => return (Image::default(), Image::default()),
        }
    } else if url.starts_with("http") {
        match ureq::get(url).call() {
            Ok(response) => {
                let mut bytes = Vec::new();
                if response.into_reader().read_to_end(&mut bytes).is_err() {
                    return (Image::default(), Image::default());
                }
                bytes
            }
            Err(e) => {
                warn!("Failed to download: {}", e);
                return (Image::default(), Image::default());
            }
        }
    } else {
        return (Image::default(), Image::default());
    };

    let img = match image::load_from_memory(&img_data) {
        Ok(i) => i,
        Err(_) => return (Image::default(), Image::default()),
    };

    use image::imageops::FilterType;
    let resized = img.resize_to_fill(256, 256, FilterType::CatmullRom);
    let _ = resized.save(&original_path);

    let blur_base = img.resize_to_fill(128, 128, FilterType::Triangle);
    let blurred = blur_base.blur(15.0);
    let _ = blurred.save(&blur_path);

    let art = Image::load_from_path(&original_path).unwrap_or_default();
    let blur = Image::load_from_path(&blur_path).unwrap_or_default();

    // Cache
    CACHED_ART_PATH.with(|p| *p.borrow_mut() = original_path.to_string_lossy().to_string());
    CACHED_ART.with(|i| *i.borrow_mut() = art.clone());
    CACHED_BLUR_PATH.with(|p| *p.borrow_mut() = blur_path.to_string_lossy().to_string());
    CACHED_BLUR.with(|i| *i.borrow_mut() = blur.clone());

    (art, blur)
}

fn media_command<F>(action: F)
where
    F: FnOnce(&mpris::Player),
{
    match PlayerFinder::new() {
        Ok(finder) => {
            if let Ok(player) = finder.find_active() {
                action(&player);
            } else {
                warn!("No active player");
            }
        }
        Err(e) => error!("PlayerFinder error: {}", e),
    }
}
