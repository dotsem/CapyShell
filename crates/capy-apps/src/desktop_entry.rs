//! Desktop entry parsing.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// parsed from .desktop files.
#[derive(Clone, Debug)]
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

/// Parse a .desktop file into a DesktopApp struct.
pub fn parse_desktop_file(path: &Path) -> Option<DesktopApp> {
    let content = fs::read_to_string(path).ok()?;
    let mut entries = HashMap::new();
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }

        if in_desktop_entry {
            if let Some((key, value)) = line.split_once('=') {
                entries.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }

    if entries.get("Type").map(|s| s.as_str()) != Some("Application") {
        return None;
    }

    let name = entries.get("Name")?.clone();
    let exec = entries.get("Exec")?.clone();
    let id = path.file_name()?.to_string_lossy().to_string();

    Some(DesktopApp {
        id,
        name,
        exec,
        icon_name: entries.get("Icon").cloned(),
        startup_wm_class: entries.get("StartupWMClass").cloned(),
        comment: entries.get("Comment").cloned(),
        categories: entries
            .get("Categories")
            .map(|s| s.split(';').map(String::from).collect())
            .unwrap_or_default(),
        keywords: entries
            .get("Keywords")
            .map(|s| s.split(';').map(String::from).collect())
            .unwrap_or_default(),
        no_display: entries
            .get("NoDisplay")
            .map(|s| s == "true")
            .unwrap_or(false),
        desktop_file_path: path.to_path_buf(),
    })
}
