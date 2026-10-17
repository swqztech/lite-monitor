use crate::monitor::{format_bytes_per_sec, HardwareMonitor, Metrics};
use crate::settings::Settings;
use eframe::egui;
use std::time::{Duration, Instant};

const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(232, 234, 237);
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(154, 160, 168);
const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(28, 30, 34);

pub struct LiteMonitorApp {
    settings: Settings,
    hw: HardwareMonitor,
    metrics: Metrics,
    display: Metrics,
    last_poll: Instant,
    show_settings: bool,
    settings_dirty: bool,
    last_window_size: [f32; 2],
}

impl LiteMonitorApp {
    pub fn new() -> Self {
        let settings = Settings::load();
        let last_window_size = settings.window_size();
        Self {
            settings,
            hw: HardwareMonitor::new(),
            metrics: Metrics::default(),
            display: Metrics::default(),
            last_poll: Instant::now() - Duration::from_secs(1),
            show_settings: false,
            settings_dirty: false,
            last_window_size,
        }
    }

    fn apply_viewport(&self, ctx: &egui::Context) {
        let alpha = (self.settings.opacity.clamp(0.35, 1.0) * 255.0) as u8;
        ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(alpha < 250));
        let level = if self.settings.always_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
    }

    fn sync_window_size(&mut self, ctx: &egui::Context) {
        let size = self.settings.window_size();
        if size != self.last_window_size {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size[0], size[1])));
            self.last_window_size = size;
        }
    }

    fn apply_style(&self, ctx: &egui::Context) {
        ctx.set_pixels_per_point(self.settings.ui_scale.clamp(1.0, 1.8));
    }

    fn poll_if_due(&mut self) {
        let interval = Duration::from_millis(self.settings.refresh_ms.max(200));
        if self.last_poll.elapsed() < interval {
            return;
        }
        self.last_poll = Instant::now();
        self.metrics = self.hw.poll();
        let t = 0.35;
        self.display.cpu_percent = lerp(self.display.cpu_percent, self.metrics.cpu_percent, t);
        self.display.mem_percent = lerp(self.display.mem_percent, self.metrics.mem_percent, t);
        if let Some(g) = self.metrics.gpu_percent {
            self.display.gpu_percent = Some(lerp(self.display.gpu_percent.unwrap_or(g), g, t));
        } else {
            self.display.gpu_percent = None;
        }
        self.display.mem_used_gb = self.metrics.mem_used_gb;
        self.display.mem_total_gb = self.metrics.mem_total_gb;
        self.display.gpu_mem_used_mb = self.metrics.gpu_mem_used_mb;
        self.display.gpu_mem_total_mb = self.metrics.gpu_mem_total_mb;
        self.display.gpu_temp_c = self.metrics.gpu_temp_c;
        self.display.disk_used_percent =
            lerp(self.display.disk_used_percent, self.metrics.disk_used_percent, t);
        self.display.net_down_bps = lerp_f64(self.display.net_down_bps, self.metrics.net_down_bps, t);
        self.display.net_up_bps = lerp_f64(self.display.net_up_bps, self.metrics.net_up_bps, t);
    }

    fn gpu_value_text(&self) -> (String, f32) {
        if let Some(gpu) = self.display.gpu_percent {
            let mem = match (self.display.gpu_mem_used_mb, self.display.gpu_mem_total_mb) {
                (Some(u), Some(t)) => format!("{:.0}%  {}/{} MB", gpu, u, t),
                _ => format!("{:.0}%", gpu),
            };
            let temp = self
                .display
                .gpu_temp_c
                .map(|t| format!("{}  {}℃", mem, t))
                .unwrap_or(mem);
            (temp, gpu)
        } else {
            ("无 NVIDIA 数据".to_string(), 0.0)
        }
    }
}

fn alpha_from_settings(settings: &Settings) -> u8 {
    (settings.opacity.clamp(0.35, 1.0) * 255.0) as u8
}

impl eframe::App for LiteMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_style(ctx);
        self.apply_viewport(ctx);
        self.sync_window_size(ctx);
        self.poll_if_due();
        if self.settings_dirty {
            self.settings.save();
            self.last_window_size = self.settings.window_size();
            self.settings_dirty = false;
        }

        let panel_w = if self.settings.vertical_layout {
            self.settings.panel_width.max(260.0)
        } else {
            self.settings.panel_width.max(520.0)
        };
        let bg = egui::Color32::from_rgba_unmultiplied(
            BG_PANEL.r(),
            BG_PANEL.g(),
            BG_PANEL.b(),
            alpha_from_settings(&self.settings),
        );

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(bg)
                    .rounding(12.0)
                    .inner_margin(egui::Margin::symmetric(14.0, 12.0)),
            )
            .show(ctx, |ui| {
                ui.set_width(panel_w);
                header_bar(ui, &mut self.show_settings);

                ui.add_space(6.0);

                if self.settings.vertical_layout {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 10.0;
                        self.draw_metrics_vertical(ui, panel_w);
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        self.draw_metrics_horizontal(ui);
                    });
                }

                let drag_rect = ui.max_rect();
                if ui
                    .interact(drag_rect, ui.id().with("drag"), egui::Sense::click_and_drag())
                    .dragged()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
            });

        if self.show_settings {
            settings_window(ctx, self);
        }

        ctx.request_repaint_after(Duration::from_millis(self.settings.refresh_ms / 2));
    }
}

impl LiteMonitorApp {
    fn draw_metrics_vertical(&self, ui: &mut egui::Ui, bar_w: f32) {
        if self.settings.show_cpu {
            metric_row(
                ui,
                bar_w,
                "CPU",
                format!("{:.0}%", self.display.cpu_percent),
                Some(self.display.cpu_percent),
            );
        }
        if self.settings.show_mem {
            metric_row(
                ui,
                bar_w,
                "内存",
                format!(
                    "{:.1} / {:.1} GB",
                    self.display.mem_used_gb,
                    self.display.mem_total_gb
                ),
                Some(self.display.mem_percent),
            );
        }
        if self.settings.show_gpu {
            let (text, pct) = self.gpu_value_text();
            metric_row(ui, bar_w, "GPU", text, if pct > 0.0 { Some(pct) } else { None });
        }
        if self.settings.show_disk {
            metric_row(
                ui,
                bar_w,
                "磁盘",
                format!("已用 {:.0}%", self.display.disk_used_percent),
                Some(self.display.disk_used_percent),
            );
        }
        if self.settings.show_net {
            net_row(
                ui,
                bar_w,
                format!("↓ {}", format_bytes_per_sec(self.display.net_down_bps)),
                format!("↑ {}", format_bytes_per_sec(self.display.net_up_bps)),
            );
        }
    }

    fn draw_metrics_horizontal(&self, ui: &mut egui::Ui) {
        if self.settings.show_cpu {
            metric_block_compact(ui, "CPU", format!("{:.0}%", self.display.cpu_percent), self.display.cpu_percent);
        }
        if self.settings.show_mem {
            metric_block_compact(
                ui,
                "内存",
                format!("{:.1}/{:.1}G", self.display.mem_used_gb, self.display.mem_total_gb),
                self.display.mem_percent,
            );
        }
        if self.settings.show_gpu {
            let (text, pct) = self.gpu_value_text();
            metric_block_compact(ui, "GPU", text, pct);
        }
        if self.settings.show_disk {
            metric_block_compact(
                ui,
                "磁盘",
                format!("{:.0}%", self.display.disk_used_percent),
                self.display.disk_used_percent,
            );
        }
        if self.settings.show_net {
            let label = format!(
                "↓{}  ↑{}",
                format_bytes_per_sec(self.display.net_down_bps),
                format_bytes_per_sec(self.display.net_up_bps)
            );
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("网络").color(TEXT_MUTED).size(12.0));
                ui.label(egui::RichText::new(label).color(TEXT_PRIMARY).size(14.0));
            });
        }
    }
}

fn header_bar(ui: &mut egui::Ui, show_settings: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("LiteMonitor")
                .strong()
                .color(TEXT_PRIMARY)
                .size(15.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("设置").size(13.0))
                        .fill(egui::Color32::from_rgb(50, 54, 60))
                        .rounding(6.0)
                        .min_size(egui::vec2(52.0, 26.0)),
                )
                .clicked()
            {
                *show_settings = !*show_settings;
            }
        });
    });
}

fn settings_window(ctx: &egui::Context, app: &mut LiteMonitorApp) {
    egui::Window::new("设置")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("界面缩放");
            if ui
                .add(egui::Slider::new(&mut app.settings.ui_scale, 1.0..=1.8).step_by(0.05))
                .changed()
            {
                app.settings_dirty = true;
            }
            if ui
                .checkbox(&mut app.settings.vertical_layout, "竖向排列（推荐）")
                .changed()
            {
                app.settings_dirty = true;
            }
            ui.label("面板宽度");
            let range = if app.settings.vertical_layout {
                260.0..=480.0
            } else {
                520.0..=1400.0
            };
            if ui.add(egui::Slider::new(&mut app.settings.panel_width, range)).changed() {
                app.settings_dirty = true;
            }
            ui.label("刷新间隔（毫秒）");
            if ui
                .add(egui::Slider::new(&mut app.settings.refresh_ms, 200..=5000).step_by(100.0))
                .changed()
            {
                app.settings_dirty = true;
            }
            ui.label("透明度");
            if ui
                .add(egui::Slider::new(&mut app.settings.opacity, 0.4..=1.0))
                .changed()
            {
                app.settings_dirty = true;
            }
            if ui.checkbox(&mut app.settings.always_on_top, "窗口置顶").changed() {
                app.settings_dirty = true;
            }
            ui.separator();
            if ui.checkbox(&mut app.settings.show_cpu, "显示 CPU").changed() {
                app.settings_dirty = true;
            }
            if ui.checkbox(&mut app.settings.show_mem, "显示内存").changed() {
                app.settings_dirty = true;
            }
            if ui.checkbox(&mut app.settings.show_gpu, "显示 GPU（NVIDIA）").changed() {
                app.settings_dirty = true;
            }
            if ui.checkbox(&mut app.settings.show_disk, "显示磁盘占用").changed() {
                app.settings_dirty = true;
            }
            if ui.checkbox(&mut app.settings.show_net, "显示网络").changed() {
                app.settings_dirty = true;
            }
            if ui.button("保存并关闭").clicked() {
                app.settings_dirty = true;
                app.show_settings = false;
            }
        });
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_f64(a: f64, b: f64, t: f32) -> f64 {
    a + (b - a) * t as f64
}

fn bar_color(percent: f32) -> egui::Color32 {
    if percent >= 90.0 {
        egui::Color32::from_rgb(239, 83, 80)
    } else if percent >= 70.0 {
        egui::Color32::from_rgb(255, 193, 7)
    } else {
        egui::Color32::from_rgb(0, 200, 83)
    }
}

/// 竖向：一行标题 + 数值 + 全宽进度条
fn metric_row(ui: &mut egui::Ui, bar_w: f32, title: &str, value: String, percent: Option<f32>) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(title).color(TEXT_MUTED).size(14.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(value).strong().color(TEXT_PRIMARY).size(16.0));
            });
        });
        if let Some(p) = percent {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 9.0), egui::Sense::hover());
            draw_bar(ui, rect, p);
        }
    });
}

fn net_row(ui: &mut egui::Ui, bar_w: f32, down: String, up: String) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new("网络").color(TEXT_MUTED).size(14.0));
        ui.horizontal(|ui| {
            ui.set_width(bar_w);
            ui.label(egui::RichText::new(down).color(TEXT_PRIMARY).size(15.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(up).color(TEXT_PRIMARY).size(15.0));
            });
        });
    });
}

fn draw_bar(ui: &mut egui::Ui, rect: egui::Rect, percent: f32) {
    let painter = ui.painter();
    painter.rect_filled(rect, 5.0, egui::Color32::from_rgb(55, 58, 64));
    let fill_w = rect.width() * (percent.clamp(0.0, 100.0) / 100.0);
    if fill_w > 0.5 {
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
        painter.rect_filled(fill, 5.0, bar_color(percent));
    }
}

fn metric_block_compact(ui: &mut egui::Ui, title: &str, value: String, percent: f32) {
    let w = 128.0;
    ui.vertical(|ui| {
        ui.set_width(w);
        ui.label(egui::RichText::new(title).color(TEXT_MUTED).size(12.0));
        ui.label(egui::RichText::new(value).color(TEXT_PRIMARY).size(14.0).strong());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 7.0), egui::Sense::hover());
        draw_bar(ui, rect, percent);
    });
}
