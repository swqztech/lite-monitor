use crate::mem_trim;
use crate::monitor::{format_bytes_per_sec, lerp_disks, AppUsage, HardwareMonitor, Metrics};
use crate::settings::Settings;
use crate::theme::{
    self, ACCENT, APP_COL_CPU, APP_COL_MEM, BAR_TRACK, BG_ROW_ALT, BORDER, BTN_IDLE, TEXT_ACCENT,
    TEXT_MUTED, TEXT_PRIMARY, TOAST_OK,
};
use eframe::egui;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum MainTab {
    Overview,
    Apps,
}

pub struct LiteMonitorApp {
    settings: Settings,
    hw: HardwareMonitor,
    metrics: Metrics,
    display: Metrics,
    last_poll: Instant,
    show_settings: bool,
    settings_dirty: bool,
    last_window_size: [f32; 2],
    toast: String,
    toast_until: Option<Instant>,
    active_tab: MainTab,
}

impl LiteMonitorApp {
    pub fn new() -> Self {
        let settings = Settings::load();
        let last_window_size = settings.window_size(2, false);
        Self {
            settings,
            hw: HardwareMonitor::new(),
            metrics: Metrics::default(),
            display: Metrics::default(),
            last_poll: Instant::now() - Duration::from_secs(1),
            show_settings: false,
            settings_dirty: false,
            last_window_size,
            toast: String::new(),
            toast_until: None,
            active_tab: MainTab::Overview,
        }
    }

    fn apps_tab_active(&self) -> bool {
        self.settings.show_apps_tab && self.active_tab == MainTab::Apps
    }

    fn window_size_now(&self) -> [f32; 2] {
        self.settings
            .window_size(self.display.disks.len().max(2), self.apps_tab_active())
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
        let size = self.window_size_now();
        if size != self.last_window_size {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size[0], size[1])));
            self.last_window_size = size;
        }
    }

    fn apply_style(&self, ctx: &egui::Context) {
        theme::apply_visuals(ctx);
        ctx.set_pixels_per_point(self.settings.ui_scale.clamp(1.0, 2.0));
        let mut style = (*ctx.style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(16.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(15.0, egui::FontFamily::Proportional),
        );
        style.spacing.item_spacing = egui::vec2(8.0, 10.0);
        ctx.set_style(style);
    }

    fn poll_if_due(&mut self) {
        let interval = Duration::from_millis(self.settings.refresh_ms.max(200));
        if self.last_poll.elapsed() < interval {
            return;
        }
        self.last_poll = Instant::now();
        let top_n = self.settings.app_top_n.clamp(5, 25) as usize;
        self.metrics = self.hw.poll(top_n);
        let t = 0.35;
        self.display.cpu_percent = lerp(self.display.cpu_percent, self.metrics.cpu_percent, t);
        self.display.mem_percent = lerp(self.display.mem_percent, self.metrics.mem_percent, t);
        if let Some(g) = self.metrics.gpu_percent {
            self.display.gpu_percent = Some(lerp(self.display.gpu_percent.unwrap_or(g), g, t));
        } else {
            self.display.gpu_percent = None;
        }
        self.display.mem_used_gb = self.metrics.mem_used_gb;
        self.display.mem_avail_gb = self.metrics.mem_avail_gb;
        self.display.mem_total_gb = self.metrics.mem_total_gb;
        self.display.gpu_mem_used_mb = self.metrics.gpu_mem_used_mb;
        self.display.gpu_mem_total_mb = self.metrics.gpu_mem_total_mb;
        self.display.gpu_temp_c = self.metrics.gpu_temp_c;
        lerp_disks(&mut self.display.disks, &self.metrics.disks, t);
        self.display.net_down_bps = lerp_f64(self.display.net_down_bps, self.metrics.net_down_bps, t);
        self.display.net_up_bps = lerp_f64(self.display.net_up_bps, self.metrics.net_up_bps, t);
        self.display.apps = self.metrics.apps.clone();
    }

    fn trim_memory(&mut self) {
        match mem_trim::purge_standby_memory() {
            Ok(r) => {
                let top_n = self.settings.app_top_n.clamp(5, 25) as usize;
                self.metrics = self.hw.poll_now(top_n);
                self.display.mem_used_gb = self.metrics.mem_used_gb;
                self.display.mem_avail_gb = self.metrics.mem_avail_gb;
                self.display.mem_total_gb = self.metrics.mem_total_gb;
                self.display.mem_percent = self.metrics.mem_percent;

                let delta = r.avail_after_gb - r.avail_before_gb;
                if delta >= 0.15 {
                    self.toast = format!(
                        "可用内存 +{:.1} GB（{:.1} → {:.1} GB）",
                        delta,
                        r.avail_before_gb,
                        r.avail_after_gb
                    );
                } else if r.any_ok {
                    self.toast = format!(
                        "已执行清理；可用 {:.1} GB（待机缓存不多时变化很小）",
                        r.avail_after_gb
                    );
                } else if r.need_admin_hint {
                    self.toast =
                        "需要管理员：请右键 LiteMonitor.exe → 以管理员身份运行，再点释放".into();
                } else {
                    self.toast = "未能清理：请以管理员身份运行后重试".into();
                }
            }
            Err(e) => self.toast = e,
        }
        self.toast_until = Some(Instant::now() + Duration::from_secs(6));
        self.last_poll = Instant::now();
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

impl eframe::App for LiteMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_style(ctx);
        self.apply_viewport(ctx);
        self.poll_if_due();
        self.sync_window_size(ctx);
        if self.settings_dirty {
            self.settings.save();
            self.last_window_size = self.window_size_now();
            self.settings_dirty = false;
        }

        if let Some(until) = self.toast_until {
            if Instant::now() > until {
                self.toast_until = None;
                self.toast.clear();
            }
        }

        let panel_w = self.settings.content_width(self.apps_tab_active());

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::panel_fill_alpha(self.settings.opacity))
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .rounding(12.0)
                    .inner_margin(egui::Margin::symmetric(16.0, 14.0)),
            )
            .show(ctx, |ui| {
                ui.set_width(panel_w);
                header_bar(
                    ui,
                    ctx,
                    &mut self.show_settings,
                    &mut self.active_tab,
                    self.settings.show_apps_tab,
                );
                if !self.toast.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(&self.toast)
                            .size(13.0)
                            .color(TOAST_OK),
                    );
                }
                ui.add_space(8.0);

                if self.apps_tab_active() {
                    self.draw_apps_tab(ui, panel_w);
                } else if self.settings.vertical_layout {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 12.0;
                        self.draw_metrics_vertical(ui, panel_w);
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        self.draw_metrics_horizontal(ui);
                    });
                }
            });

        if self.show_settings {
            settings_window(ctx, self);
        }

        ctx.request_repaint_after(Duration::from_millis(self.settings.refresh_ms / 2));
    }
}

impl LiteMonitorApp {
    fn draw_metrics_vertical(&mut self, ui: &mut egui::Ui, bar_w: f32) {
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
            let mut trim = false;
            memory_row(
                ui,
                bar_w,
                format!(
                    "已用 {:.1} · 可用 {:.1} / {:.1} GB",
                    self.display.mem_used_gb,
                    self.display.mem_avail_gb,
                    self.display.mem_total_gb
                ),
                self.display.mem_percent,
                || trim = true,
            );
            if trim {
                self.trim_memory();
            }
        }
        if self.settings.show_gpu {
            let (text, pct) = self.gpu_value_text();
            metric_row(ui, bar_w, "GPU", text, if pct > 0.0 { Some(pct) } else { None });
        }
        if self.settings.show_disk {
            if self.display.disks.is_empty() {
                metric_row(ui, bar_w, "磁盘", "未检测到分区".to_string(), None);
            } else {
                for d in &self.display.disks {
                    metric_row(
                        ui,
                        bar_w,
                        &d.label,
                        format!(
                            "已用 {:.0}% · 剩余 {:.1} GB",
                            d.used_percent,
                            d.free_gb
                        ),
                        Some(d.used_percent),
                    );
                }
            }
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

    fn draw_apps_tab(&self, ui: &mut egui::Ui, bar_w: f32) {
        ui.label(
            egui::RichText::new("应用程序（按内存占用排序）")
                .color(TEXT_MUTED)
                .size(14.0),
        );
        ui.add_space(8.0);
        apps_table_header(ui, bar_w);
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(bar_w);
                if self.display.apps.is_empty() {
                    ui.label(egui::RichText::new("正在采集…").color(TEXT_MUTED));
                    return;
                }
                for (i, app) in self.display.apps.iter().enumerate() {
                    app_row(ui, bar_w, app, i % 2 == 0);
                    ui.add_space(2.0);
                }
            });
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
            for d in &self.display.disks {
                metric_block_compact(
                    ui,
                    &d.label,
                    format!("{:.0}%", d.used_percent),
                    d.used_percent,
                );
            }
        }
        if self.settings.show_net {
            let label = format!(
                "↓{}  ↑{}",
                format_bytes_per_sec(self.display.net_down_bps),
                format_bytes_per_sec(self.display.net_up_bps)
            );
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("网络").color(TEXT_MUTED).size(14.0));
                ui.label(egui::RichText::new(label).color(TEXT_PRIMARY).size(16.0));
            });
        }
    }
}

fn header_bar(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    show_settings: &mut bool,
    active_tab: &mut MainTab,
    show_apps_tab: bool,
) {
    ui.horizontal(|ui| {
        let title = ui.add(
            egui::Label::new(
                egui::RichText::new("LiteMonitor")
                    .strong()
                    .color(TEXT_ACCENT)
                    .size(18.0),
            )
            .sense(egui::Sense::click_and_drag()),
        );
        if title.dragged() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if show_apps_tab {
            ui.add_space(8.0);
            tab_chip(ui, "概览", *active_tab == MainTab::Overview, || *active_tab = MainTab::Overview);
            tab_chip(ui, "应用", *active_tab == MainTab::Apps, || *active_tab = MainTab::Apps);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("设置").size(14.0))
                        .fill(BTN_IDLE)
                        .rounding(8.0)
                        .min_size(egui::vec2(56.0, 28.0)),
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
                .add(egui::Slider::new(&mut app.settings.ui_scale, 1.0..=2.0).step_by(0.05))
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
                360.0..=720.0
            } else {
                520.0..=1400.0
            };
            ui.label("应用页宽度至少 540px；调宽面板可同时加宽应用列表。");
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
            ui.label("内存行上的「释放」会清理系统待机列表；若无效请以管理员身份运行。");
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
            if ui.checkbox(&mut app.settings.show_disk, "显示各磁盘分区").changed() {
                app.settings_dirty = true;
            }
            if ui.checkbox(&mut app.settings.show_net, "显示网络").changed() {
                app.settings_dirty = true;
            }
            if ui.checkbox(&mut app.settings.show_apps_tab, "显示「应用」标签").changed() {
                app.settings_dirty = true;
            }
            ui.label("应用列表条数");
            if ui
                .add(egui::Slider::new(&mut app.settings.app_top_n, 5..=25).step_by(1.0))
                .changed()
            {
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

fn metric_row(ui: &mut egui::Ui, bar_w: f32, title: &str, value: String, percent: Option<f32>) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(title).color(TEXT_MUTED).size(16.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(value).strong().color(TEXT_PRIMARY).size(18.0));
            });
        });
        if let Some(p) = percent {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 10.0), egui::Sense::hover());
            draw_bar(ui, rect, p);
        }
    });
}

fn memory_row(
    ui: &mut egui::Ui,
    bar_w: f32,
    value: String,
    percent: f32,
    on_trim: impl FnOnce(),
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("内存").color(TEXT_MUTED).size(16.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("释放").size(13.0).color(TEXT_PRIMARY))
                            .fill(ACCENT)
                            .rounding(6.0)
                            .min_size(egui::vec2(48.0, 26.0)),
                    )
                    .on_hover_text("清理待机内存（与系统「内存压缩」缓存类似）")
                    .clicked()
                {
                    on_trim();
                }
                ui.add_space(8.0);
                ui.label(egui::RichText::new(value).strong().color(TEXT_PRIMARY).size(18.0));
            });
        });
        let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 10.0), egui::Sense::hover());
        draw_bar(ui, rect, percent);
    });
}

fn net_row(ui: &mut egui::Ui, bar_w: f32, down: String, up: String) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new("网络").color(TEXT_MUTED).size(16.0).strong());
        ui.horizontal(|ui| {
            ui.set_width(bar_w);
            ui.label(egui::RichText::new(down).color(TEXT_PRIMARY).size(17.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(up).color(TEXT_PRIMARY).size(17.0));
            });
        });
    });
}

fn draw_bar(ui: &mut egui::Ui, rect: egui::Rect, percent: f32) {
    let painter = ui.painter();
    painter.rect_filled(rect, 5.0, BAR_TRACK);
    let fill_w = rect.width() * (percent.clamp(0.0, 100.0) / 100.0);
    if fill_w > 0.5 {
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
        painter.rect_filled(fill, 5.0, theme::bar_color(percent));
    }
}

fn tab_chip(ui: &mut egui::Ui, label: &str, selected: bool, on_click: impl FnOnce()) {
    let fill = if selected { ACCENT } else { BTN_IDLE };
    let stroke = if selected {
        egui::Stroke::new(1.0_f32, TEXT_ACCENT.gamma_multiply(0.6))
    } else {
        egui::Stroke::NONE
    };
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new(label)
                    .size(13.0)
                    .color(if selected { TEXT_PRIMARY } else { TEXT_MUTED }),
            )
            .fill(fill)
            .stroke(stroke)
            .rounding(8.0)
            .min_size(egui::vec2(48.0, 28.0)),
        )
        .clicked()
    {
        on_click();
    }
}

fn apps_table_header(ui: &mut egui::Ui, total_w: f32) {
    let name_w = total_w - APP_COL_CPU - APP_COL_MEM - 8.0;
    ui.horizontal(|ui| {
        ui.add_sized(
            [name_w, 22.0],
            egui::Label::new(egui::RichText::new("名称").color(TEXT_MUTED).size(13.0)),
        );
        ui.add_sized(
            [APP_COL_CPU, 22.0],
            egui::Label::new(
                egui::RichText::new("CPU")
                    .color(TEXT_MUTED)
                    .size(13.0),
            ),
        );
        ui.add_sized(
            [APP_COL_MEM, 22.0],
            egui::Label::new(
                egui::RichText::new("内存")
                    .color(TEXT_MUTED)
                    .size(13.0),
            ),
        );
    });
}

fn format_mem_mb(mb: f32) -> String {
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else if mb >= 1.0 {
        format!("{:.0} MB", mb)
    } else {
        format!("{:.1} MB", mb)
    }
}

fn app_row(ui: &mut egui::Ui, total_w: f32, app: &AppUsage, alt: bool) {
    let name = if app.instances > 1 {
        format!("{} ×{}", app.name, app.instances)
    } else {
        app.name.clone()
    };
    let name_w = total_w - APP_COL_CPU - APP_COL_MEM - 8.0;
    let row_h = 28.0;
    let bg = if alt { BG_ROW_ALT } else { egui::Color32::TRANSPARENT };

    egui::Frame::none()
        .fill(bg)
        .rounding(6.0)
        .inner_margin(egui::Margin::symmetric(6.0, 4.0))
        .show(ui, |ui| {
            ui.set_width(total_w);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(name_w, row_h),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(name)
                                    .color(TEXT_PRIMARY)
                                    .size(14.0),
                            )
                            .truncate(),
                        );
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(APP_COL_CPU, row_h),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.label(
                            egui::RichText::new(format!("{:.1}%", app.cpu_percent))
                                .color(if app.cpu_percent >= 30.0 {
                                    theme::BAR_WARN
                                } else {
                                    TEXT_MUTED
                                })
                                .size(14.0),
                        );
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(APP_COL_MEM, row_h),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.label(
                            egui::RichText::new(format_mem_mb(app.mem_mb))
                                .color(TEXT_ACCENT)
                                .size(14.0)
                                .strong(),
                        );
                    },
                );
            });
        });
}

fn metric_block_compact(ui: &mut egui::Ui, title: &str, value: String, percent: f32) {
    let w = 128.0;
    ui.vertical(|ui| {
        ui.set_width(w);
        ui.label(egui::RichText::new(title).color(TEXT_MUTED).size(14.0));
        ui.label(egui::RichText::new(value).color(TEXT_PRIMARY).size(15.0).strong());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 8.0), egui::Sense::hover());
        draw_bar(ui, rect, percent);
    });
}
