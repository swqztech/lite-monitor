//! 当前用户开机自启（注册表 Run）

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const VALUE_NAME: &str = "LiteMonitor";

/// 根据设置写入或删除自启项
pub fn apply(enabled: bool) -> Result<(), String> {
    set_enabled(enabled)
}

#[cfg(windows)]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    use std::io;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_READ)
        .map_err(|e| format!("打开自启注册表失败: {}", e))?;

    if !enabled {
        match run.delete_value(VALUE_NAME) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("取消自启失败: {}", e)),
        }
    } else {
        let exe = std::env::current_exe().map_err(|e| format!("无法定位程序路径: {}", e))?;
        let cmd = format!("\"{}\"", exe.display());
        run.set_value(VALUE_NAME, &cmd)
            .map_err(|e| format!("设置自启失败: {}", e))?;
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) -> Result<(), String> {
    Err("仅支持 Windows".into())
}

#[cfg(windows)]
pub fn is_enabled() -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ);
    match run {
        Ok(run) => run.get_value::<String, _>(VALUE_NAME).is_ok(),
        Err(_) => false,
    }
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}
