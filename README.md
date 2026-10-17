# lite-monitor

自用版 Windows 硬件监控横条，**Rust + egui**（非 Tauri）。界面与能力参考 [Diorser/LiteMonitor](https://github.com/Diorser/LiteMonitor)，实现独立、体量更小。

## 功能（v0.1）

| 模块 | 说明 |
|------|------|
| CPU | 使用率 + 三色进度条 |
| 内存 | 已用/总量 + 占用率 |
| GPU | NVIDIA（NVML）：占用、显存、温度；无 NV 卡显示 — |
| 磁盘 | **每个分区单独一行**（如 磁盘 C:、磁盘 D:） |
| 内存 | 行内 **「释放」** 清理待机内存（部分环境需管理员运行） |
| 网络 | 全网卡合计上传/下载速率 |

- 窗口置顶、透明度、刷新间隔、显示项开关
- 设置写入 `%APPDATA%/lite-monitor/settings.json`
- **默认竖向排列**：每项一行、全宽进度条，字号更大（可在设置里调「界面缩放」）
- 设置里可切回横向横条；拖动窗口任意区域可移动

## 构建

```bash
cargo build --release
target\release\lite-monitor.exe
```

## 打包

```bash
bash packaging/build.sh
```

## 与原版差异

未实现：任务栏嵌入、主题 JSON、插件、FPS、历史曲线、网页版、内存清理等。后续可按需加。

## 协议

MIT（与参考项目一致思路；本仓库代码独立编写）
