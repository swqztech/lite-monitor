use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub refresh_ms: u64,
    pub always_on_top: bool,
    pub opacity: f32,
    pub panel_width: f32,
    pub show_cpu: bool,
    pub show_mem: bool,
    pub show_gpu: bool,
    pub show_disk: bool,
    pub show_net: bool,
    pub clamp_to_screen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            refresh_ms: 1000,
            always_on_top: true,
            opacity: 0.92,
            panel_width: 780.0,
            show_cpu: true,
            show_mem: true,
            show_gpu: true,
            show_disk: true,
            show_net: true,
            clamp_to_screen: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = settings_path();
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(s) = serde_json::from_str(&data) {
                return s;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }
}

fn settings_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("lite-monitor").join("settings.json")
}
