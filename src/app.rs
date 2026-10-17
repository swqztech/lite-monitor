use crate::monitor::{format_bytes_per_sec, HardwareMonitor, Metrics};
use crate::settings::Settings;
use eframe::egui;
use std::time::{Duration, Instant};

pub struct LiteMonitorApp {
    settings: Settings,
    hw: HardwareMonitor,
    metrics: Metrics,
    display: Metrics,
    last_poll: Instant,
    show_settings: bool,
    settings_dirty: bool,
}

impl LiteMonitorApp {
    pub fn new() -> Self {
        Self {
            settings: Settings::load(),
            hw: HardwareMonitor::new(),
            metrics: Metrics::default(),
            display: Metrics::default(),
            last_poll: Instant::now() - Duration::from_secs(1),
            show_settings: false,
            settings_dirty: false,
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

    fn poll_if_due(&mut self) {
        let interval = Duration::from_millis(self.settings.refresh_ms.max(200));
        if self.last_poll.elapsed() < interval {
            return;
        }
        self.last_poll = Instant::now();
        self.metrics = self.hw.poll();
        // 简单平滑，减轻跳变
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
}

fn alpha_from_settings(settings: &Settings) -> u8 {
    (settings.opacity.clamp(0.35, 1.0) * 255.0) as u8
}

impl eframe::App for LiteMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_viewport(ctx);
        self.poll_if_due();
        if self.settings_dirty {
            self.settings.save();
            self.settings_dirty = false;
        }

        let panel_w = self.settings.panel_width.max(480.0);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(32, 34, 37, alpha_from_settings(&self.settings)))
                    .rounding(10.0)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.set_width(panel_w);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if self.settings.show_cpu {
                        metric_block(
                            ui,
                            "CPU",
                            format!("{:.0}%", self.display.cpu_percent),
                            self.display.cpu_percent,
                        );
                    }
                    if self.settings.show_mem {
                        metric_block(
                            ui,
                            "内存",
                            format!(
                                "{:.1}/{:.1}G",
                                self.display.mem_used_gb, self.display.mem_total_gb
                            ),
                            self.display.mem_percent,
                        );
                    }
                    if self.settings.show_gpu {
                        if let Some(gpu) = self.display.gpu_percent {
                            let mem = match (
                                self.display.gpu_mem_used_mb,
                                self.display.gpu_mem_total_mb,
                            ) {
                                (Some(u), Some(t)) => format!("{:.0}% · {}/{}M", gpu, u, t),
                                _ => format!("{:.0}%", gpu),
                            };
                            let temp = self
                                .display
                                .gpu_temp_c
                                .map(|t| format!(" · {}℃", t))
                                .unwrap_or_default();
                            metric_block(ui, "GPU", format!("{}{}", mem, temp), gpu);
                        } else {
                            metric_block(ui, "GPU", "—".to_string(), 0.0);
                        }
                    }
                    if self.settings.show_disk {
                        metric_block(
                            ui,
                            "磁盘",
                            format!("{:.0}% 已用", self.display.disk_used_percent),
                            self.display.disk_used_percent,
                        );
                    }
                    if self.settings.show_net {
                        let label = format!(
                            "↓{} ↑{}",
                            format_bytes_per_sec(self.display.net_down_bps),
                            format_bytes_per_sec(self.display.net_up_bps)
                        );
                        disk_net_block(ui, "网络", label);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("⚙").on_hover_text("设置").clicked() {
                            self.show_settings = !self.show_settings;
                        }
                    });
                });

                // 标题区拖动窗口
                let rect = ui.max_rect();
                if ui.interact(rect, ui.id().with("drag"), egui::Sense::click_and_drag()).dragged()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
            });

        if self.show_settings {
            egui::Window::new("设置")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("刷新间隔（毫秒）");
                    if ui
                        .add(egui::Slider::new(&mut self.settings.refresh_ms, 200..=5000).step_by(100.0))
                        .changed()
                    {
                        self.settings_dirty = true;
                    }
                    ui.label("面板宽度");
                    if ui
                        .add(egui::Slider::new(&mut self.settings.panel_width, 480.0..=1400.0))
                        .changed()
                    {
                        self.settings_dirty = true;
                    }
                    ui.label("透明度");
                    if ui
                        .add(egui::Slider::new(&mut self.settings.opacity, 0.4..=1.0))
                        .changed()
                    {
                        self.settings_dirty = true;
                    }
                    if ui.checkbox(&mut self.settings.always_on_top, "窗口置顶").changed() {
                        self.settings_dirty = true;
                    }
                    if ui.checkbox(&mut self.settings.clamp_to_screen, "限制在屏幕内").changed() {
                        self.settings_dirty = true;
                    }
                    ui.separator();
                    if ui.checkbox(&mut self.settings.show_cpu, "显示 CPU").changed() {
                        self.settings_dirty = true;
                    }
                    if ui.checkbox(&mut self.settings.show_mem, "显示内存").changed() {
                        self.settings_dirty = true;
                    }
                    if ui.checkbox(&mut self.settings.show_gpu, "显示 GPU（NVIDIA / NVML）").changed() {
                        self.settings_dirty = true;
                    }
                    if ui.checkbox(&mut self.settings.show_disk, "显示磁盘 IO").changed() {
                        self.settings_dirty = true;
                    }
                    if ui.checkbox(&mut self.settings.show_net, "显示网络").changed() {
                        self.settings_dirty = true;
                    }
                    if ui.button("保存并关闭").clicked() {
                        self.settings_dirty = true;
                        self.show_settings = false;
                    }
                });
        }

        ctx.request_repaint_after(Duration::from_millis(self.settings.refresh_ms / 2));
    }
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

fn metric_block(ui: &mut egui::Ui, title: &str, value: String, percent: f32) {
    let w = 118.0;
    ui.vertical(|ui| {
        ui.set_width(w);
        ui.label(egui::RichText::new(title).small().color(egui::Color32::from_gray(160)));
        ui.label(egui::RichText::new(value).strong());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 6.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 3.0, egui::Color32::from_gray(55));
        let fill_w = rect.width() * (percent.clamp(0.0, 100.0) / 100.0);
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
        painter.rect_filled(fill, 3.0, bar_color(percent));
    });
}

fn disk_net_block(ui: &mut egui::Ui, title: &str, value: String) {
    let w = 150.0;
    ui.vertical(|ui| {
        ui.set_width(w);
        ui.label(egui::RichText::new(title).small().color(egui::Color32::from_gray(160)));
        ui.label(egui::RichText::new(value).size(13.0));
    });
}
