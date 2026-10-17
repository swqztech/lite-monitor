//! 界面配色
use eframe::egui;

pub const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(22, 26, 34);
pub const BG_ELEVATED: egui::Color32 = egui::Color32::from_rgb(30, 36, 48);
pub const BG_ROW_ALT: egui::Color32 = egui::Color32::from_rgb(28, 33, 44);
pub const BORDER: egui::Color32 = egui::Color32::from_rgb(48, 56, 72);

pub const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(236, 240, 248);
pub const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(130, 142, 162);
pub const TEXT_ACCENT: egui::Color32 = egui::Color32::from_rgb(125, 211, 252);

pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(59, 130, 246);
pub const ACCENT_SOFT: egui::Color32 = egui::Color32::from_rgb(37, 99, 235);
pub const BTN_IDLE: egui::Color32 = egui::Color32::from_rgb(42, 48, 62);

pub const BAR_TRACK: egui::Color32 = egui::Color32::from_rgb(40, 46, 58);
pub const BAR_OK: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);
pub const BAR_WARN: egui::Color32 = egui::Color32::from_rgb(251, 191, 36);
pub const BAR_DANGER: egui::Color32 = egui::Color32::from_rgb(248, 113, 113);

pub const TOAST_OK: egui::Color32 = egui::Color32::from_rgb(110, 231, 183);

/// 应用列表列宽
pub const APP_COL_CPU: f32 = 62.0;
pub const APP_COL_MEM: f32 = 80.0;
pub const APPS_PANEL_MIN_WIDTH: f32 = 540.0;

pub fn apply_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_ELEVATED;
    visuals.extreme_bg_color = BG_PANEL;
    visuals.faint_bg_color = BG_ROW_ALT;
    visuals.widgets.noninteractive.bg_fill = BG_ELEVATED;
    visuals.widgets.inactive.bg_fill = BTN_IDLE;
    visuals.widgets.inactive.fg_stroke.color = TEXT_PRIMARY;
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(52, 60, 78);
    visuals.widgets.active.bg_fill = ACCENT_SOFT;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.55);
    visuals.window_stroke.color = BORDER;
    ctx.set_visuals(visuals);
}

pub fn bar_color(percent: f32) -> egui::Color32 {
    if percent >= 90.0 {
        BAR_DANGER
    } else if percent >= 70.0 {
        BAR_WARN
    } else {
        BAR_OK
    }
}

pub fn panel_fill_alpha(opacity: f32) -> egui::Color32 {
    let a = (opacity.clamp(0.35, 1.0) * 255.0) as u8;
    egui::Color32::from_rgba_unmultiplied(BG_PANEL.r(), BG_PANEL.g(), BG_PANEL.b(), a)
}
