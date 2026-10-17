use nvml_wrapper::Nvml;
use std::time::{Duration, Instant};
use sysinfo::{Disk, Disks, Networks, System};

#[derive(Clone, Debug)]
pub struct DiskInfo {
    /// 如「磁盘 C:」
    pub label: String,
    pub used_percent: f32,
    pub free_gb: f32,
    pub total_gb: f32,
}

/// 展示用指标（已平滑）
#[derive(Clone, Default)]
pub struct Metrics {
    pub cpu_percent: f32,
    pub mem_used_gb: f32,
    pub mem_total_gb: f32,
    pub mem_percent: f32,
    pub gpu_percent: Option<f32>,
    pub gpu_mem_used_mb: Option<u64>,
    pub gpu_mem_total_mb: Option<u64>,
    pub gpu_temp_c: Option<u32>,
    pub disks: Vec<DiskInfo>,
    pub net_down_bps: f64,
    pub net_up_bps: f64,
}

struct RateSample {
    at: Instant,
    value: u64,
}

pub struct HardwareMonitor {
    sys: System,
    networks: Networks,
    disks: Disks,
    nvml: Option<Nvml>,
    last_net_rx: Option<RateSample>,
    last_net_tx: Option<RateSample>,
    cpu_warm: bool,
}

impl HardwareMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let nvml = Nvml::init().ok();
        Self {
            sys,
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            nvml,
            last_net_rx: None,
            last_net_tx: None,
            cpu_warm: false,
        }
    }

    pub fn poll(&mut self) -> Metrics {
        if !self.cpu_warm {
            std::thread::sleep(Duration::from_millis(120));
            self.cpu_warm = true;
        }
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.networks.refresh(true);
        self.disks.refresh(true);

        let cpu_percent = self.sys.global_cpu_usage();

        let mem_total = self.sys.total_memory();
        let mem_used = self.sys.used_memory();
        let mem_total_gb = mem_total as f32 / (1024.0 * 1024.0 * 1024.0);
        let mem_used_gb = mem_used as f32 / (1024.0 * 1024.0 * 1024.0);
        let mem_percent = if mem_total > 0 {
            (mem_used as f32 / mem_total as f32) * 100.0
        } else {
            0.0
        };

        let (net_down_bps, net_up_bps) = self.network_rates();
        let disks = self.list_disks();
        let (gpu_percent, gpu_mem_used_mb, gpu_mem_total_mb, gpu_temp_c) = self.gpu_stats();

        Metrics {
            cpu_percent,
            mem_used_gb,
            mem_total_gb,
            mem_percent,
            gpu_percent,
            gpu_mem_used_mb,
            gpu_mem_total_mb,
            gpu_temp_c,
            disks,
            net_down_bps,
            net_up_bps,
        }
    }

    fn network_rates(&mut self) -> (f64, f64) {
        let now = Instant::now();
        let mut rx: u64 = 0;
        let mut tx: u64 = 0;
        for (_name, data) in self.networks.iter() {
            rx += data.total_received();
            tx += data.total_transmitted();
        }
        let down = rate_bps(&mut self.last_net_rx, rx, now);
        let up = rate_bps(&mut self.last_net_tx, tx, now);
        (down, up)
    }

    fn list_disks(&self) -> Vec<DiskInfo> {
        let mut out = Vec::new();
        for disk in self.disks.iter() {
            if !should_show_disk(disk) {
                continue;
            }
            let total = disk.total_space();
            let avail = disk.available_space();
            let used = total.saturating_sub(avail);
            let used_percent = if total > 0 {
                (used as f32 / total as f32) * 100.0
            } else {
                0.0
            };
            let gb = 1024.0 * 1024.0 * 1024.0;
            out.push(DiskInfo {
                label: disk_label(disk),
                used_percent,
                free_gb: avail as f32 / gb,
                total_gb: total as f32 / gb,
            });
        }
        out.sort_by(|a, b| a.label.cmp(&b.label));
        out
    }

    fn gpu_stats(&self) -> (Option<f32>, Option<u64>, Option<u64>, Option<u32>) {
        let Some(nvml) = self.nvml.as_ref() else {
            return (None, None, None, None);
        };
        let Ok(device) = nvml.device_by_index(0) else {
            return (None, None, None, None);
        };
        let Ok(util) = device.utilization_rates() else {
            return (None, None, None, None);
        };
        let Ok(mem) = device.memory_info() else {
            return (None, None, None, None);
        };
        let temp = device
            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .ok();
        (
            Some(util.gpu as f32),
            Some(mem.used / (1024 * 1024)),
            Some(mem.total / (1024 * 1024)),
            temp,
        )
    }
}

fn should_show_disk(disk: &Disk) -> bool {
    let total = disk.total_space();
    if total == 0 {
        return false;
    }
    let mp = disk.mount_point().to_string_lossy();
    if mp.is_empty() {
        return false;
    }
    // 跳过 UNC 网络盘
    if mp.starts_with("\\\\") {
        return false;
    }
    true
}

fn disk_label(disk: &Disk) -> String {
    let mp = disk.mount_point().to_string_lossy();
    let letter = mp.trim_end_matches('\\');
    if letter.len() <= 3 && letter.ends_with(':') {
        format!("磁盘 {}", letter)
    } else {
        format!("磁盘 {}", letter)
    }
}

fn rate_bps(last: &mut Option<RateSample>, value: u64, now: Instant) -> f64 {
    let bps = if let Some(prev) = last {
        let dt = now.duration_since(prev.at).as_secs_f64();
        if dt > 0.0 {
            (value.saturating_sub(prev.value) as f64) / dt
        } else {
            0.0
        }
    } else {
        0.0
    };
    *last = Some(RateSample { at: now, value });
    bps
}

pub fn format_bytes_per_sec(bps: f64) -> String {
    if bps >= 1024.0 * 1024.0 {
        format!("{:.1} MB/s", bps / (1024.0 * 1024.0))
    } else if bps >= 1024.0 {
        format!("{:.0} KB/s", bps / 1024.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

pub fn lerp_disks(display: &mut Vec<DiskInfo>, target: &[DiskInfo], t: f32) {
    for m in target {
        if let Some(d) = display.iter_mut().find(|d| d.label == m.label) {
            d.used_percent = lerp(d.used_percent, m.used_percent, t);
            d.free_gb = m.free_gb;
            d.total_gb = m.total_gb;
        } else {
            display.push(m.clone());
        }
    }
    display.retain(|d| target.iter().any(|m| m.label == d.label));
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
