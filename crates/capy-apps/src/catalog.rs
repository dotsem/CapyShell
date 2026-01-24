//! App Catalog implementation.

use crate::desktop_entry::{DesktopApp, parse_desktop_file};
use crate::icons::IconTheme;
use crate::paths::{get_application_directories, load_cache_from_disk, save_cache_to_disk};
use log::info;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

/// Events emitted when the catalog changes.
#[derive(Debug, Clone)]
pub enum AppEvent {
    Refresh,
}

/// The main application catalog.
pub struct AppCatalog {
    /// Desktop applications indexed by ID (e.g. "firefox.desktop").
    apps: RwLock<HashMap<String, DesktopApp>>,
    /// Apps indexed by StartupWMClass (lowercase).
    apps_by_wm_class: RwLock<HashMap<String, String>>,
    /// Icon theme handler.
    icon_theme: IconTheme,
    /// Cached icon lookups.
    icon_cache: RwLock<HashMap<String, Option<PathBuf>>>,
    /// Event sender for catalog updates.
    event_tx: tokio::sync::broadcast::Sender<AppEvent>,
}

impl AppCatalog {
    /// Create a new empty catalog.
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(16);

        Self {
            apps: RwLock::new(HashMap::new()),
            apps_by_wm_class: RwLock::new(HashMap::new()),
            icon_theme: IconTheme::new(),
            icon_cache: RwLock::new(HashMap::new()),
            event_tx: tx,
        }
    }

    /// Refresh the catalog (scan apps and icons).
    /// This runs asynchronously in a background thread.
    pub fn refresh(&self) {
        // Clone Arc reference for the thread if possible, but self is &self here.
        // We'll need to structure this so the catalog itself can be shared.
        // For now, let's assume the caller handles Arc wrapping or we use internal mutability.

        // Actually, internal components handle their own locking, so this is fine.
        // But scanning is heavy, so we should spawn a thread.
        // However, we need 'static lifetime or Arc capability.
        // For simplicity in this step, let's make it synchronous logic wrapped by thread in lib.rs or main.

        // Wait, self.scan() needs mutability? No, we use RwLock.
        self.scan_internal();
    }

    fn scan_internal(&self) {
        info!("Scanning app catalog...");

        self.icon_theme.build_index();

        self.scan_desktop_files();

        if let Some(cache) = load_cache_from_disk() {
            let mut guard = self.icon_cache.write().unwrap();
            *guard = cache;
        }

        self.prepopulate_cache();

        // Notify listeners
        let _ = self.event_tx.send(AppEvent::Refresh);

        info!("App catalog refresh complete.");
    }

    /// Resolve an icon path by name.
    pub fn resolve_icon(&self, name: &str) -> Option<PathBuf> {
        if name.is_empty() {
            return None;
        }

        let key = name.to_lowercase();

        {
            let cache = self.icon_cache.read().unwrap();
            if let Some(cached) = cache.get(&key) {
                return cached.clone();
            }
        }

        let result = self.icon_theme.resolve(name);

        {
            let mut cache = self.icon_cache.write().unwrap();
            cache.insert(key, result.clone());
        }

        result
    }

    /// Get app details by ID.
    pub fn get_app(&self, id: &str) -> Option<DesktopApp> {
        self.apps.read().unwrap().get(id).cloned()
    }

    /// Get app ID by StartupWMClass.
    pub fn get_app_id_by_wm_class(&self, wm_class: &str) -> Option<String> {
        self.apps_by_wm_class
            .read()
            .unwrap()
            .get(&wm_class.to_lowercase())
            .cloned()
    }

    /// Subscribe to catalog changes.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<AppEvent> {
        self.event_tx.subscribe()
    }

    fn scan_desktop_files(&self) {
        let dirs = get_application_directories();
        let mut new_apps = HashMap::new();
        let mut new_wm_classes = HashMap::new();

        for dir in dirs {
            if !dir.exists() {
                continue;
            }

            let walker = walkdir::WalkDir::new(&dir).follow_links(true).max_depth(3);
            for entry in walker.into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                    if let Some(app) = parse_desktop_file(path) {
                        let id = app.id.clone();

                        if let Some(wm_class) = &app.startup_wm_class {
                            new_wm_classes.insert(wm_class.to_lowercase(), id.clone());
                        }

                        // Fallback: index by basename as well (common convention)
                        let basename = id.trim_end_matches(".desktop").to_lowercase();
                        new_wm_classes.entry(basename).or_insert(id.clone());

                        new_apps.insert(id, app);
                    }
                }
            }
        }

        *self.apps.write().unwrap() = new_apps;
        *self.apps_by_wm_class.write().unwrap() = new_wm_classes;
    }

    fn prepopulate_cache(&self) {
        let apps = self.apps.read().unwrap();

        for app in apps.values() {
            if let Some(icon_name) = &app.icon_name {
                self.resolve_icon(icon_name);
            }
        }

        let cache = self.icon_cache.read().unwrap();
        save_cache_to_disk(&*cache);
    }
}
