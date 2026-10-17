//! Windows 待机列表 / 工作集清理

#[derive(Debug, Clone)]
pub struct TrimResult {
    pub avail_before_gb: f32,
    pub avail_after_gb: f32,
    /// 至少有一种方式返回成功
    pub any_ok: bool,
    pub need_admin_hint: bool,
}

#[cfg(windows)]
pub fn purge_standby_memory() -> Result<TrimResult, String> {
    let before = physical_avail_bytes();
    enable_privilege("SeProfileSingleProcessPrivilege");
    enable_privilege("SeDebugPrivilege");

    let mut any_ok = false;
    let mut denied = false;

    // 顺序：轻量 → 彻底（EmptyWorkingSets 需管理员）
    for cmd in [5i32, 4, 3, 2] {
        match purge_command(cmd) {
            Ok(true) => any_ok = true,
            Ok(false) => {}
            Err(code) if code == -1073741790 => denied = true, // STATUS_ACCESS_DENIED
            Err(_) => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
    }

    trim_current_process();
    std::thread::sleep(std::time::Duration::from_millis(150));
    let after = physical_avail_bytes();

    let gb = |b: u64| b as f32 / (1024.0 * 1024.0 * 1024.0);
    Ok(TrimResult {
        avail_before_gb: gb(before),
        avail_after_gb: gb(after),
        any_ok,
        need_admin_hint: !any_ok && denied,
    })
}

#[cfg(not(windows))]
pub fn purge_standby_memory() -> Result<TrimResult, String> {
    Err("仅支持 Windows".into())
}

#[cfg(windows)]
fn physical_avail_bytes() -> u64 {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut st = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        if GlobalMemoryStatusEx(&mut st).is_ok() {
            return st.ullAvailPhys;
        }
    }
    0
}

#[cfg(windows)]
fn trim_current_process() {
    use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(windows)]
fn enable_privilege(name: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return;
        }

        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut luid = LUID::default();
        if LookupPrivilegeValueW(PCWSTR::null(), PCWSTR(name_w.as_ptr()), &mut luid).is_err() {
            let _ = CloseHandle(token);
            return;
        }

        let mut tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let _ = AdjustTokenPrivileges(token, false, Some(&mut tp), 0, None, None);
        let _ = CloseHandle(token);
    }
}

/// NTSTATUS；成功时 Ok(true)，失败 Ok(false) 或 Err(status)
#[cfg(windows)]
fn purge_command(command: i32) -> Result<bool, i32> {
    use std::ffi::c_void;

    #[link(name = "ntdll")]
    extern "system" {
        fn NtSetSystemInformation(
            system_information_class: u32,
            system_information: *mut c_void,
            system_information_length: u32,
        ) -> i32;
    }

    const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 80;

    // 官方结构仅一个 Command 字段（此前多传 flags 会导致调用无效）
    #[repr(C)]
    struct MemoryListCommand {
        command: i32,
    }

    let mut info = MemoryListCommand { command };
    let status = unsafe {
        NtSetSystemInformation(
            SYSTEM_MEMORY_LIST_INFORMATION,
            (&mut info as *mut MemoryListCommand).cast(),
            std::mem::size_of::<MemoryListCommand>() as u32,
        )
    };
    if status == 0 {
        Ok(true)
    } else if status < 0 {
        Err(status)
    } else {
        Ok(false)
    }
}
