use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn default_vertical_layout() -> bool {
    true
}

fn default_ui_scale() -> f32 {
    1.35
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub refresh_ms: u64,
    pub always_on_top: bool,
    pub opacity: f32,
    pub panel_width: f32,
    /// 竖向排列（默认）；关闭则为横向横条
    #[serde(default = "default_vertical_layout")]
    pub vertical_layout: bool,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
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
            opacity: 0.95,
            panel_width: 340.0,
            vertical_layout: true,
            ui_scale: 1.35,
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
    pub fn visible_row_count(&self, disk_count: usize) -> u32 {
        let mut n = 0u32;
        if self.show_cpu {
            n += 1;
        }
        if self.show_mem {
            n += 1;
        }
        if self.show_gpu {
            n += 1;
        }
        if self.show_disk {
            n += disk_count.max(1) as u32;
        }
        if self.show_net {
            n += 1;
        }
        n.max(1)
    }

    pub fn panel_height(&self, disk_count: usize) -> f32 {
        if self.vertical_layout {
            let rows = self.visible_row_count(disk_count);
            56.0 + rows as f32 * 66.0 + 20.0
        } else {
            72.0
        }
    }

    pub fn window_size(&self, disk_count: usize) -> [f32; 2] {
        if self.vertical_layout {
            [self.panel_width.max(280.0), self.panel_height(disk_count)]
        } else {
            [self.panel_width.max(520.0), 72.0]
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
