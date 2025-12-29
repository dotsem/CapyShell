//! MPRIS media player monitoring service.
//!
//! Uses zbus for D-Bus signal monitoring (Seeked, PropertiesChanged).
//! Uses mpris crate for querying player state.

use crate::panels::taskbar::events;
use image::imageops::FilterType;
use log::{error, info, warn};
use mpris::{PlaybackStatus, PlayerFinder};
use sha2::{Digest, Sha256};
use slint::SharedString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct MprisData {
    pub title: SharedString,
    pub artist: SharedString,
    pub album_art_path: SharedString,
    pub blurred_art_path: SharedString,
    pub length_secs: f32,
    pub position_secs: f32,
    pub is_playing: bool,
    pub has_media: bool,
    pub is_track_change: bool, // True = clear in-memory image cache, show placeholder
}

const CACHE_DIR: &str = ".cache/CapyShell/thumbs";
const BLUR_SIGMA: f32 = 15.0;

// Generation counter to handle race conditions for async image loading
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn start() {
    std::thread::Builder::new()
        .name("mpris-monitor".to_string())
        .spawn(|| {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!("Failed to create tokio runtime for MPRIS: {}", e);
                    return;
                }
            };
            rt.block_on(run_mpris_loop());
        })
        .expect("Failed to spawn mpris thread");
}

async fn run_mpris_loop() {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let cache_dir = PathBuf::from(home).join(CACHE_DIR);
    if let Err(e) = fs::create_dir_all(&cache_dir) {
        error!("Failed to create cache dir: {}", e);
        return;
    }

    info!("Starting MPRIS D-Bus event listener with zbus");

    loop {
        // Try to connect with zbus D-Bus monitoring
        match dbus_worker(&cache_dir).await {
            Ok(()) => {
                info!("D-Bus worker exited normally, restarting...");
            }
            Err(e) => {
                warn!("D-Bus worker failed: {}. Retrying in 2s...", e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }

        // Clear state when worker exits
        GENERATION.fetch_add(1, Ordering::SeqCst);
        events::send_mpris(MprisData::default());
    }
}

/// D-Bus signal monitoring using zbus
async fn dbus_worker(cache_dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::stream::StreamExt;
    use zbus::Connection;

    let connection = Connection::session().await?;

    // Find current MPRIS player bus name
    let player_bus_name = find_mpris_player(&connection).await?;
    info!("Connected to MPRIS player: {}", player_bus_name);

    // Listen for PropertiesChanged on the player interface
    let props_rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .sender(player_bus_name.as_str())?
        .path("/org/mpris/MediaPlayer2")?
        .build();

    // Listen for Seeked signal
    let seeked_rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.mpris.MediaPlayer2.Player")?
        .member("Seeked")?
        .sender(player_bus_name.as_str())?
        .path("/org/mpris/MediaPlayer2")?
        .build();

    let mut props_stream =
        zbus::MessageStream::for_match_rule(props_rule, &connection, Some(100)).await?;
    let mut seeked_stream =
        zbus::MessageStream::for_match_rule(seeked_rule, &connection, Some(100)).await?;

    info!("Listening for MPRIS D-Bus signals (PropertiesChanged + Seeked)...");

    // Send initial state
    send_full_update(cache_dir);

    // Track last title to detect track changes
    let mut last_title: Option<String> = None;

    loop {
        tokio::select! {
            Some(msg) = props_stream.next() => {
                if let Ok(msg) = msg {
                    handle_properties_changed(&msg, cache_dir, &mut last_title);
                }
            }
            Some(msg) = seeked_stream.next() => {
                if let Ok(msg) = msg {
                    handle_seeked_signal(&msg);
                }
            }
            else => {
                info!("Signal streams ended");
                break;
            }
        }
    }

    Ok(())
}

/// Find an active MPRIS player on the session bus
async fn find_mpris_player(
    connection: &zbus::Connection,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let dbus_proxy = zbus::fdo::DBusProxy::new(connection).await?;
    let names = dbus_proxy.list_names().await?;

    // Find MPRIS players, prefer spotify
    let mpris_players: Vec<_> = names
        .iter()
        .filter(|n| n.as_str().starts_with("org.mpris.MediaPlayer2."))
        .collect();

    if mpris_players.is_empty() {
        return Err("No MPRIS players found".into());
    }

    // Prefer spotify, otherwise use first available
    let player = mpris_players
        .iter()
        .find(|n| n.as_str().contains("spotify"))
        .unwrap_or(&mpris_players[0]);

    Ok(player.to_string())
}

/// Handle PropertiesChanged signal
fn handle_properties_changed(
    msg: &zbus::Message,
    cache_dir: &Path,
    last_title: &mut Option<String>,
) {
    // Try to parse the signal body to determine what changed
    // Body format: (interface: str, changed_props: dict, invalidated: array)
    let body: Result<
        (
            String,
            std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            Vec<String>,
        ),
        _,
    > = msg.body().deserialize();

    if let Ok((interface, changed_props, _)) = body {
        if interface != "org.mpris.MediaPlayer2.Player" {
            return;
        }

        // Check if PlaybackStatus changed
        if changed_props.contains_key("PlaybackStatus") {
            info!("PlaybackStatus changed");
            send_playback_update();
            return;
        }

        // Check if Metadata changed (track change)
        if changed_props.contains_key("Metadata") {
            info!("Metadata changed (track change)");

            // Get current title to detect actual track changes
            let current_title = get_current_title();
            let is_new_track = match (last_title.as_ref(), &current_title) {
                (Some(old), Some(new)) => old != new,
                (None, Some(_)) => true,
                _ => false,
            };

            if is_new_track {
                *last_title = current_title;
                send_full_update(cache_dir);
            }
            return;
        }

        // For any other property change, just update playback state
        info!("Other property changed");
        send_playback_update();
    }
}

/// Handle Seeked signal
fn handle_seeked_signal(msg: &zbus::Message) {
    // Seeked signal body: (position_in_us: i64)
    let body: Result<(i64,), _> = msg.body().deserialize();

    if let Ok((position_us,)) = body {
        let position_secs = position_us as f32 / 1_000_000.0;
        info!("🎯 Seeked signal received! Position: {:.1}s", position_secs);
        send_position_update(position_secs);
    } else {
        warn!("Failed to parse Seeked signal body");
    }
}

/// Get current title from player
fn get_current_title() -> Option<String> {
    PlayerFinder::new()
        .ok()?
        .find_active()
        .ok()?
        .get_metadata()
        .ok()?
        .title()
        .map(|s| s.to_string())
}

/// Send full state update (initial connection or track change)
fn send_full_update(cache_dir: &Path) {
    let finder = match PlayerFinder::new() {
        Ok(f) => f,
        Err(_) => return,
    };

    let player = match finder.find_active() {
        Ok(p) => p,
        Err(_) => return,
    };

    let metadata = match player.get_metadata() {
        Ok(m) => m,
        Err(_) => return,
    };

    let title = metadata.title().unwrap_or("Unknown").to_string();
    let artists = metadata.artists().map(|a| a.join(", ")).unwrap_or_default();
    let art_url = metadata.art_url().unwrap_or("").to_string();
    let length = metadata.length().unwrap_or(Duration::ZERO);

    let is_playing = player
        .get_playback_status()
        .map(|s| s == PlaybackStatus::Playing)
        .unwrap_or(false);
    let position = player.get_position().unwrap_or(Duration::ZERO);

    // Increment generation to invalidate pending image loads
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    // Send immediate update with metadata but NO art (shows placeholder)
    let immediate_data = MprisData {
        title: title.clone().into(),
        artist: artists.clone().into(),
        album_art_path: "".into(),
        blurred_art_path: "".into(),
        length_secs: length.as_secs_f32(),
        position_secs: position.as_secs_f32(),
        is_playing,
        has_media: true,
        is_track_change: true, // Signal to clear in-memory cache
    };
    events::send_mpris(immediate_data);

    // Spawn async image loading
    if !art_url.is_empty() {
        let cache_dir_clone = cache_dir.to_path_buf();
        let title_clone = title;
        let artists_clone = artists;
        let length_secs = length.as_secs_f32();

        std::thread::spawn(move || {
            // Check if still current generation before processing
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }

            if let Some((art_path, blur_path)) = process_album_art(&art_url, &cache_dir_clone) {
                // Check again after processing
                if GENERATION.load(Ordering::SeqCst) != generation {
                    return;
                }

                // Get fresh position
                let (fresh_position, fresh_playing) = get_fresh_player_state();

                let data_with_art = MprisData {
                    title: title_clone.into(),
                    artist: artists_clone.into(),
                    album_art_path: art_path.into(),
                    blurred_art_path: blur_path.into(),
                    length_secs,
                    position_secs: fresh_position,
                    is_playing: fresh_playing,
                    has_media: true,
                    is_track_change: false, // Art loaded, not a track change
                };
                events::send_mpris(data_with_art);
            }
        });
    }
}

/// Send playback state change (play/pause)
fn send_playback_update() {
    let finder = match PlayerFinder::new() {
        Ok(f) => f,
        Err(_) => return,
    };

    let player = match finder.find_active() {
        Ok(p) => p,
        Err(_) => return,
    };

    let metadata = match player.get_metadata() {
        Ok(m) => m,
        Err(_) => return,
    };

    let is_playing = player
        .get_playback_status()
        .map(|s| s == PlaybackStatus::Playing)
        .unwrap_or(false);

    let title = metadata.title().unwrap_or("Unknown").to_string();
    let artists = metadata.artists().map(|a| a.join(", ")).unwrap_or_default();
    let length = metadata.length().unwrap_or(Duration::ZERO);
    let position = player.get_position().unwrap_or(Duration::ZERO);

    // Send with empty art paths - adapter will preserve cached images
    let data = MprisData {
        title: title.into(),
        artist: artists.into(),
        album_art_path: "".into(),
        blurred_art_path: "".into(),
        length_secs: length.as_secs_f32(),
        position_secs: position.as_secs_f32(),
        is_playing,
        has_media: true,
        is_track_change: false, // Playback change, preserve art
    };
    events::send_mpris(data);
}

/// Send position update (seek)
fn send_position_update(position_secs: f32) {
    let finder = match PlayerFinder::new() {
        Ok(f) => f,
        Err(_) => return,
    };

    let player = match finder.find_active() {
        Ok(p) => p,
        Err(_) => return,
    };

    let metadata = match player.get_metadata() {
        Ok(m) => m,
        Err(_) => return,
    };

    let is_playing = player
        .get_playback_status()
        .map(|s| s == PlaybackStatus::Playing)
        .unwrap_or(false);

    let title = metadata.title().unwrap_or("Unknown").to_string();
    let artists = metadata.artists().map(|a| a.join(", ")).unwrap_or_default();
    let length = metadata.length().unwrap_or(Duration::ZERO);

    let data = MprisData {
        title: title.into(),
        artist: artists.into(),
        album_art_path: "".into(),
        blurred_art_path: "".into(),
        length_secs: length.as_secs_f32(),
        position_secs,
        is_playing,
        has_media: true,
        is_track_change: false, // Seek, preserve art
    };
    events::send_mpris(data);
}

/// Get fresh player state for async callbacks
fn get_fresh_player_state() -> (f32, bool) {
    PlayerFinder::new()
        .ok()
        .and_then(|f| f.find_active().ok())
        .map(|p| {
            let pos = p.get_position().unwrap_or(Duration::ZERO).as_secs_f32();
            let playing = p
                .get_playback_status()
                .map(|s| s == PlaybackStatus::Playing)
                .unwrap_or(false);
            (pos, playing)
        })
        .unwrap_or((0.0, false))
}

fn process_album_art(url: &str, cache_dir: &Path) -> Option<(String, String)> {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hex::encode(hasher.finalize());

    let original_path = cache_dir.join(format!("{}.png", hash));
    let blur_path = cache_dir.join(format!("{}_blur.png", hash));

    // Return cached paths if they exist
    if original_path.exists() && blur_path.exists() {
        return Some((
            original_path.to_string_lossy().to_string(),
            blur_path.to_string_lossy().to_string(),
        ));
    }

    // Fetch Image
    let img_data = if url.starts_with("file://") {
        let path = url.strip_prefix("file://").unwrap();
        fs::read(path).ok()?
    } else if url.starts_with("http") {
        match ureq::get(url).call() {
            Ok(response) => {
                let mut bytes = Vec::new();
                if response.into_reader().read_to_end(&mut bytes).is_err() {
                    return None;
                }
                bytes
            }
            Err(e) => {
                warn!("Failed to download album art from {}: {}", url, e);
                return None;
            }
        }
    } else {
        return None;
    };

    let img = image::load_from_memory(&img_data).ok()?;

    // Use CatmullRom for quality
    let resized = img.resize_to_fill(256, 256, FilterType::CatmullRom);
    resized.save(&original_path).ok()?;

    // Process Blur
    let blur_base = img.resize_to_fill(128, 128, FilterType::Triangle);
    let blurred = blur_base.blur(BLUR_SIGMA);
    blurred.save(&blur_path).ok()?;

    Some((
        original_path.to_string_lossy().to_string(),
        blur_path.to_string_lossy().to_string(),
    ))
}
