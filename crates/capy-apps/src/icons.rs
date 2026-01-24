//! Icon theme handling and indexing.

use crate::paths::{get_icon_base_directories, get_icon_theme_order, parse_icon_theme_index};
use log::debug;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::RwLock;

/// Handles icon lookups across multiple themes and directories.
pub struct IconTheme {
    /// Index of icon name (lowercase, no ext) -> path.
    index: RwLock<HashMap<String, PathBuf>>,
}

impl IconTheme {
    pub fn new() -> Self {
        Self {
            index: RwLock::new(HashMap::new()),
        }
    }

    /// Build the index of all icons.
    /// This respects theme inheritance by inserting in order (first match wins).
    pub fn build_index(&self) {
        let mut index = HashMap::new();
        let allowed_extensions: HashSet<&str> = ["png", "svg", "xpm", "webp"].into_iter().collect();

        let search_dirs = self.get_search_directories();
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
                let ext = match path.extension().and_then(|e| e.to_str()) {
                    Some(e) => e.to_lowercase(),
                    None => continue,
                };

                if !allowed_extensions.contains(ext.as_str()) {
                    continue;
                }

                let stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(s) => s.to_lowercase(),
                    None => continue,
                };

                index.entry(stem).or_insert_with(|| path.to_path_buf());
            }
        }

        *self.index.write().unwrap() = index;
    }

    /// Resolve an icon name to a path using the index.
    pub fn resolve(&self, name: &str) -> Option<PathBuf> {
        if name.starts_with('/') {
            let path = PathBuf::from(name);
            if path.exists() {
                return Some(path);
            }
        }

        let index = self.index.read().unwrap();
        let name_lower = name.to_lowercase();

        if let Some(path) = index.get(&name_lower) {
            return Some(path.clone());
        }

        // Try variations (e.g. "firefox-nightly" -> "firefox")
        // Note: For full compliance we should strip dashes accurately, but this covers 90%
        let variations = [name_lower.replace(' ', "-"), name_lower.replace('_', "-")];

        for variant in &variations {
            if let Some(path) = index.get(variant) {
                return Some(path.clone());
            }
        }

        None
    }

    fn get_search_directories(&self) -> Vec<PathBuf> {
        let mut result = Vec::new();
        let icon_dirs = get_icon_base_directories();
        let theme_order = get_icon_theme_order();

        for theme in &theme_order {
            for base_dir in &icon_dirs {
                let theme_root = base_dir.join(theme);
                if !theme_root.exists() {
                    continue;
                }

                if let Some(parsed) = parse_icon_theme_index(&theme_root) {
                    for relative in &parsed.directories {
                        result.push(theme_root.join(relative));
                    }
                } else {
                    // Fallback for directories without index.theme
                    result.push(theme_root);
                }
            }
        }

        // Always search base directories (pixmaps, icons root)
        for dir in &icon_dirs {
            result.push(dir.clone());
        }

        result
    }
}
