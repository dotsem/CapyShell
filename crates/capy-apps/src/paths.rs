//! Path helpers for XDG directories and config files.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;

/// Get base icon directories (XDG + Flatpak + Snap).
pub fn get_icon_base_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();

    let xdg_data_home =
        std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home));
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());

    // User icons
    dirs.push(PathBuf::from(&xdg_data_home).join("icons"));
    dirs.push(PathBuf::from(&home).join(".icons"));

    // System icons
    for data_dir in xdg_data_dirs.split(':') {
        if !data_dir.is_empty() {
            dirs.push(PathBuf::from(data_dir).join("icons"));
            dirs.push(PathBuf::from(data_dir).join("pixmaps"));
        }
    }

    // Standard fallback
    dirs.push(PathBuf::from("/usr/share/pixmaps"));

    // App formats (flatpak, snap)
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/icons"));
    dirs.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share/icons"));
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/icons"));

    dirs
}

/// Get all application .desktop file directories.
pub fn get_application_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();
    let xdg_data_home =
        std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home));
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());

    dirs.push(PathBuf::from(&xdg_data_home).join("applications"));

    for data_dir in xdg_data_dirs.split(':') {
        if !data_dir.is_empty() {
            dirs.push(PathBuf::from(data_dir).join("applications"));
        }
    }

    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share/applications"));
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

    dirs
}

/// Get cache path for icon cache.
pub fn get_cache_path() -> PathBuf {
    let xdg_cache = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/.cache", home)
    });

    let path = PathBuf::from(xdg_cache).join("CapyShell");
    fs::create_dir_all(&path).ok();
    path.join("icon_cache.json")
}

/// Load cache from disk.
pub fn load_cache_from_disk() -> Option<std::collections::HashMap<String, Option<PathBuf>>> {
    let path = get_cache_path();
    if !path.exists() {
        return None;
    }

    let file = fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    serde_json::from_reader(reader).ok()
}

/// Save cache to disk.
/// Cache is typically stored in ~/.cache/CapyShell/icon_cache.json
pub fn save_cache_to_disk(cache: &std::collections::HashMap<String, Option<PathBuf>>) {
    if let Ok(file) = fs::File::create(get_cache_path()) {
        let _ = serde_json::to_writer(file, cache);
    }
}

/// Parsed index.theme content.
pub struct ParsedIconTheme {
    pub directories: Vec<String>,
    pub inherits: Vec<String>,
}

pub fn parse_icon_theme_index(theme_root: &PathBuf) -> Option<ParsedIconTheme> {
    let content = fs::read_to_string(theme_root.join("index.theme")).ok()?;
    let mut directories = Vec::new();
    let mut inherits = Vec::new();
    let mut section = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.to_string();
            continue;
        }

        if section.eq_ignore_ascii_case("[Icon Theme]") {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "Directories" {
                    directories = v.split(',').map(|s| s.trim().to_string()).collect();
                } else if k.trim() == "Inherits" {
                    inherits = v.split(',').map(|s| s.trim().to_string()).collect();
                }
            }
        }
    }

    Some(ParsedIconTheme {
        directories,
        inherits,
    })
}

/// Get ordered list of icon themes from system config.
pub fn get_icon_theme_order() -> Vec<String> {
    let mut themes = Vec::new();
    let _home = std::env::var("HOME").unwrap_or_default();

    if let Ok(theme) = std::env::var("GTK_THEME") {
        themes.push(theme);
    }

    // TODO: enable a 'config-parsing' feature for robust INI parsing.
    themes.push("Adwaita".to_string());
    themes.push("hicolor".to_string());

    resolve_theme_inheritance(themes)
}

fn resolve_theme_inheritance(start_themes: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from(start_themes);
    let base_dirs = get_icon_base_directories();

    while let Some(theme) = queue.pop_front() {
        if visited.contains(&theme) {
            continue;
        }
        visited.insert(theme.clone());
        result.push(theme.clone());

        for base in &base_dirs {
            if let Some(parsed) = parse_icon_theme_index(&base.join(&theme)) {
                for parent in parsed.inherits {
                    if !visited.contains(&parent) {
                        queue.push_back(parent);
                    }
                }
                break; // Only parse first found theme instance
            }
        }
    }

    result
}
