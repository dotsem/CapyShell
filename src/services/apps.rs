//! App catalog and icon service for CapyShell.
//!
//! Provides a unified service for:
//! - Icon lookup with comprehensive directory scanning
//! - Desktop application catalog (for future app launcher)

use log::{debug, info};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use std::thread;

/// Desktop application entry parsed from .desktop files.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon_name: Option<String>,
    pub startup_wm_class: Option<String>,
    pub comment: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub no_display: bool,
    pub desktop_file_path: PathBuf,
}

/// Global app catalog singleton.
static CATALOG: OnceLock<Arc<AppCatalog>> = OnceLock::new();

/// App catalog with icon cache and desktop apps.
struct AppCatalog {
    /// Complete icon index: name (lowercase, without extension) -> path
    icon_index: RwLock<HashMap<String, PathBuf>>,
    /// Cached icon lookups (includes failed lookups as None)
    lookup_cache: RwLock<HashMap<String, Option<PathBuf>>>,
    /// Desktop applications indexed by ID.
    apps: RwLock<HashMap<String, DesktopApp>>,
    /// Apps indexed by StartupWMClass (lowercase).
    apps_by_wm_class: RwLock<HashMap<String, String>>,
    /// Whether icon indexing has completed.
    initialized: std::sync::atomic::AtomicBool,
}

impl AppCatalog {
    fn new() -> Self {
        Self {
            icon_index: RwLock::new(HashMap::new()),
            lookup_cache: RwLock::new(HashMap::new()),
            apps: RwLock::new(HashMap::new()),
            apps_by_wm_class: RwLock::new(HashMap::new()),
            initialized: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

fn get_catalog() -> Arc<AppCatalog> {
    CATALOG.get_or_init(|| Arc::new(AppCatalog::new())).clone()
}

/// Start background indexing of apps and icons.
/// Call this once from services::start_all().
pub fn start_indexing() {
    info!("Starting app catalog background indexing...");

    // Initialize the catalog
    let catalog = get_catalog();

    // Spawn background thread to build icon index and scan desktop files
    thread::spawn(move || {
        // Build icon index first (this is the comprehensive scan)
        build_icon_index(&catalog);

        // Then scan desktop files
        scan_desktop_files(&catalog);

        let index = catalog.icon_index.read().unwrap();
        let apps = catalog.apps.read().unwrap();
        info!(
            "App catalog indexing complete: {} icons, {} apps",
            index.len(),
            apps.len()
        );

        // Drop the locks before doing more work
        drop(index);
        drop(apps);

        // Mark as initialized and clear any stale cache entries
        catalog
            .initialized
            .store(true, std::sync::atomic::Ordering::SeqCst);
        {
            let mut cache = catalog.lookup_cache.write().unwrap();
            cache.clear();
        }

        // Trigger workspace updates to refresh icons in UI
        crate::services::wm::trigger_refresh();
    });
}

/// Get icon path for an app class/name.
/// Returns immediately from cache if available.
pub fn get_icon(name: &str) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }

    let catalog = get_catalog();
    let key = name.to_lowercase();

    // Check lookup cache first (for repeated lookups)
    {
        let cache = catalog.lookup_cache.read().unwrap();
        if let Some(cached) = cache.get(&key) {
            return cached.clone();
        }
    }

    // Look up in icon index
    let result = lookup_icon_internal(&catalog, &key);

    // Only cache results after initialization to avoid caching failed lookups
    // when the index hasn't been built yet
    if catalog
        .initialized
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        let mut cache = catalog.lookup_cache.write().unwrap();
        cache.insert(key, result.clone());
    }

    result
}

/// Internal icon lookup using the pre-built index.
fn lookup_icon_internal(catalog: &AppCatalog, name: &str) -> Option<PathBuf> {
    let index = catalog.icon_index.read().unwrap();

    // Handle absolute paths
    if name.starts_with('/') {
        let path = PathBuf::from(name);
        if path.exists() {
            return Some(path);
        }
    }

    // Handle home directory expansion
    let expanded_name;
    if name.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            expanded_name = name.replacen('~', &home, 1);
            let path = PathBuf::from(&expanded_name);
            if path.exists() {
                return Some(path);
            }
        }
    }

    let name_lower = name.to_lowercase();

    // Try direct lookup
    if let Some(path) = index.get(&name_lower) {
        return Some(path.clone());
    }

    // Try without extension if present
    let name_no_ext = name_lower
        .rsplit_once('.')
        .map(|(base, _)| base)
        .unwrap_or(&name_lower);

    if name_no_ext != name_lower {
        if let Some(path) = index.get(name_no_ext) {
            return Some(path.clone());
        }
    }

    // Try variations
    let variations = [
        name_lower.replace(' ', "-"),
        name_lower.replace('_', "-"),
        name_lower.replace('-', "_"),
    ];

    for variant in &variations {
        if let Some(path) = index.get(variant) {
            return Some(path.clone());
        }
    }

    None
}

/// Build comprehensive icon index by scanning all icon directories.
fn build_icon_index(catalog: &AppCatalog) {
    let mut index = HashMap::new();
    let allowed_extensions: HashSet<&str> = ["png", "svg", "xpm", "webp"].into_iter().collect();

    // Get all icon search directories (respecting theme inheritance)
    let search_dirs = get_icon_search_directories();

    debug!("Scanning {} icon directories...", search_dirs.len());

    for dir_path in search_dirs {
        if !dir_path.exists() {
            continue;
        }

        let walker = walkdir::WalkDir::new(&dir_path)
            .follow_links(true)
            .max_depth(10);

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() && !entry.file_type().is_symlink() {
                continue;
            }

            let path = entry.path();

            // Check extension
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e.to_lowercase(),
                None => continue,
            };

            if !allowed_extensions.contains(ext.as_str()) {
                continue;
            }

            // Get base name without extension
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_lowercase(),
                None => continue,
            };

            // Also index with full filename
            let full_name = match path.file_name().and_then(|s| s.to_str()) {
                Some(s) => s.to_lowercase(),
                None => continue,
            };

            // Only insert if not already present (first found wins, respects theme order)
            index.entry(stem).or_insert_with(|| path.to_path_buf());
            index.entry(full_name).or_insert_with(|| path.to_path_buf());
        }
    }

    // Store the index
    let mut catalog_index = catalog.icon_index.write().unwrap();
    *catalog_index = index;
}

/// Get ordered list of icon search directories, respecting theme inheritance.
fn get_icon_search_directories() -> Vec<PathBuf> {
    let mut result = Vec::new();
    let icon_dirs = get_icon_base_directories();
    let theme_order = get_icon_theme_order();

    // For each theme in priority order, add its directories
    for theme in &theme_order {
        for base_dir in &icon_dirs {
            let theme_root = base_dir.join(theme);
            if !theme_root.exists() {
                continue;
            }

            // Parse index.theme to get subdirectories
            if let Some(parsed) = parse_icon_theme_index(&theme_root) {
                for relative in &parsed.directories {
                    let resolved = theme_root.join(relative);
                    if resolved.exists() {
                        result.push(resolved);
                    }
                }
            } else {
                // No index.theme, add all subdirectories
                if let Ok(entries) = fs::read_dir(&theme_root) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        if entry.path().is_dir() {
                            result.push(entry.path());
                        }
                    }
                }
            }

            // Also add theme root itself
            result.push(theme_root);
        }
    }

    // Add base icon directories themselves (for non-themed icons)
    for dir in &icon_dirs {
        if dir.exists() {
            result.push(dir.clone());
        }
    }

    result
}

/// Get base icon directories (XDG + Flatpak + Snap).
fn get_icon_base_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();

    let xdg_data_home =
        std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home));
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());

    // User icons
    dirs.push(PathBuf::from(&xdg_data_home).join("icons"));
    dirs.push(PathBuf::from(&home).join(".icons"));

    // System icons from XDG_DATA_DIRS
    for data_dir in xdg_data_dirs.split(':') {
        if !data_dir.is_empty() {
            dirs.push(PathBuf::from(data_dir).join("icons"));
            dirs.push(PathBuf::from(data_dir).join("pixmaps"));
        }
    }

    // Standard paths
    dirs.push(PathBuf::from("/usr/share/icons"));
    dirs.push(PathBuf::from("/usr/share/pixmaps"));

    // Flatpak icons
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/icons"));
    dirs.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share/icons"));

    // Snap icons
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/icons"));

    dirs
}

/// Get ordered list of icon themes to search (with inheritance).
fn get_icon_theme_order() -> Vec<String> {
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    // Start with preferred themes
    for theme in get_preferred_icon_themes() {
        queue.push_back(theme);
    }

    // BFS through theme inheritance
    while let Some(theme) = queue.pop_front() {
        let theme = theme.trim().to_string();
        if theme.is_empty() || visited.contains(&theme) {
            continue;
        }

        visited.insert(theme.clone());
        order.push(theme.clone());

        // Add inherited themes
        for inherited in get_theme_inheritances(&theme) {
            if !visited.contains(&inherited) {
                queue.push_back(inherited);
            }
        }
    }

    // Always include hicolor as fallback
    if !order.contains(&"hicolor".to_string()) {
        order.push("hicolor".to_string());
    }

    order
}

/// Get preferred icon themes from system configuration.
fn get_preferred_icon_themes() -> Vec<String> {
    let mut themes = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();

    // Environment variables
    if let Ok(theme) = std::env::var("XDG_ICON_THEME") {
        themes.push(theme.split(':').next().unwrap_or("").to_string());
    }
    if let Ok(theme) = std::env::var("GTK_THEME") {
        themes.push(theme.split(':').next().unwrap_or("").to_string());
    }

    // GTK 3/4 settings
    for path in [
        format!("{}/.config/gtk-3.0/settings.ini", home),
        format!("{}/.config/gtk-4.0/settings.ini", home),
    ] {
        if let Some(theme) = read_ini_value(&path, "Settings", "gtk-icon-theme-name") {
            themes.push(theme);
        }
    }

    // GTK 2 settings
    if let Some(theme) = read_gtkrc_theme(&format!("{}/.gtkrc-2.0", home)) {
        themes.push(theme);
    }

    // KDE settings
    if let Some(theme) = read_ini_value(&format!("{}/.config/kdeglobals", home), "Icons", "Theme") {
        themes.push(theme);
    }

    // Add fallbacks
    themes.push("hicolor".to_string());
    themes.push("Adwaita".to_string());
    themes.push("breeze".to_string());
    themes.push("Papirus".to_string());

    themes
}

/// Get inherited themes for a given theme.
fn get_theme_inheritances(theme_name: &str) -> Vec<String> {
    let icon_dirs = get_icon_base_directories();
    let mut inherits = Vec::new();

    for base_dir in &icon_dirs {
        let theme_root = base_dir.join(theme_name);
        if let Some(parsed) = parse_icon_theme_index(&theme_root) {
            inherits.extend(parsed.inherits);
        }
    }

    inherits
}

/// Parsed icon theme index.theme file.
struct ParsedIconTheme {
    directories: Vec<String>,
    inherits: Vec<String>,
}

/// Parse index.theme file for a theme.
fn parse_icon_theme_index(theme_root: &PathBuf) -> Option<ParsedIconTheme> {
    let index_file = theme_root.join("index.theme");
    let content = fs::read_to_string(&index_file).ok()?;

    let mut directories = Vec::new();
    let mut inherits = Vec::new();
    let mut current_section: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = Some(line[1..line.len() - 1].trim().to_string());
            continue;
        }

        let is_icon_theme = current_section
            .as_ref()
            .is_some_and(|s| s.eq_ignore_ascii_case("Icon Theme"));

        if !is_icon_theme {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key.eq_ignore_ascii_case("Directories") {
                directories.extend(
                    value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                );
            } else if key.eq_ignore_ascii_case("Inherits") {
                inherits.extend(
                    value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                );
            }
        }
    }

    Some(ParsedIconTheme {
        directories,
        inherits,
    })
}

/// Read a value from an INI-style file.
fn read_ini_value(path: &str, section: &str, key: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let mut current_section: Option<&str> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = Some(&line[1..line.len() - 1]);
            continue;
        }

        if !current_section.is_some_and(|s| s.eq_ignore_ascii_case(section)) {
            continue;
        }

        if let Some((k, v)) = line.split_once('=') {
            if k.trim().eq_ignore_ascii_case(key) {
                return Some(v.trim().to_string());
            }
        }
    }

    None
}

/// Read icon theme from GTK 2 style rc file.
fn read_gtkrc_theme(path: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;

    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with("gtk-icon-theme-name") {
            continue;
        }

        if let Some((_, value)) = line.split_once('=') {
            let value = value.trim().trim_matches('"');
            return Some(value.to_string());
        }
    }

    None
}

/// Scan desktop files and populate the app catalog.
fn scan_desktop_files(catalog: &AppCatalog) {
    let dirs = get_application_directories();
    let mut apps_count = 0;

    for dir in dirs {
        if !dir.exists() {
            continue;
        }

        let walker = walkdir::WalkDir::new(&dir).follow_links(true).max_depth(3);

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.extension().is_some_and(|e| e == "desktop") {
                continue;
            }

            if let Some(app) = parse_desktop_file(path) {
                let id = app.id.clone();

                // Index by WM class if present
                if let Some(ref wm_class) = app.startup_wm_class {
                    let mut by_class = catalog.apps_by_wm_class.write().unwrap();
                    by_class.insert(wm_class.to_lowercase(), id.clone());
                }

                // Also index by app ID basename
                let basename = id.trim_end_matches(".desktop").to_lowercase();
                {
                    let mut by_class = catalog.apps_by_wm_class.write().unwrap();
                    by_class.insert(basename, id.clone());
                }

                let mut apps = catalog.apps.write().unwrap();
                apps.insert(id, app);
                apps_count += 1;
            }
        }
    }

    debug!("Indexed {} desktop applications", apps_count);
}

/// Get list of directories containing .desktop files.
fn get_application_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();

    let xdg_data_home =
        std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home));
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());

    // User applications
    dirs.push(PathBuf::from(&xdg_data_home).join("applications"));
    dirs.push(PathBuf::from(&home).join(".local/share/applications"));

    // System applications
    for data_dir in xdg_data_dirs.split(':') {
        if !data_dir.is_empty() {
            dirs.push(PathBuf::from(data_dir).join("applications"));
        }
    }

    // Flatpak applications
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share/applications"));

    // Snap applications
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

    dirs
}

/// Parse a .desktop file into a DesktopApp.
fn parse_desktop_file(path: &std::path::Path) -> Option<DesktopApp> {
    let content = fs::read_to_string(path).ok()?;
    let mut entries = HashMap::new();
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            entries.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    // Must be an Application type
    let app_type = entries.get("Type")?;
    if !app_type.eq_ignore_ascii_case("Application") {
        return None;
    }

    // Must have Name and Exec
    let name = entries
        .get("Name")
        .or_else(|| entries.get("Name[en]"))?
        .clone();
    let exec = entries.get("Exec")?.clone();

    let id = path.file_name()?.to_str()?.to_string();

    Some(DesktopApp {
        id,
        name,
        exec,
        icon_name: entries.get("Icon").cloned(),
        startup_wm_class: entries
            .get("StartupWMClass")
            .or_else(|| entries.get("StartupWmClass"))
            .cloned(),
        comment: entries.get("Comment").cloned(),
        categories: entries
            .get("Categories")
            .map(|c| {
                c.split(';')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        keywords: entries
            .get("Keywords")
            .map(|k| {
                k.split(';')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        no_display: entries
            .get("NoDisplay")
            .is_some_and(|v| v.eq_ignore_ascii_case("true")),
        desktop_file_path: path.to_path_buf(),
    })
}
