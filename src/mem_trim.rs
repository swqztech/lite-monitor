//! Windows 待机列表清理（与常见「释放内存」工具同类机制）

#[cfg(windows)]
pub fn purge_standby_memory() -> Result<(), String> {
    enable_profile_privilege()?;
    purge_via_nt()?;
    trim_current_process();
    Ok(())
}

#[cfg(not(windows))]
pub fn purge_standby_memory() -> Result<(), String> {
    Err("仅支持 Windows".into())
}

#[cfg(windows)]
fn trim_current_process() {
    use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(windows)]
fn enable_profile_privilege() -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .map_err(|e| format!("无法打开进程令牌: {}", e))?;

        let name: Vec<u16> = "SeProfileSingleProcessPrivilege"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut luid = LUID::default();
        LookupPrivilegeValueW(PCWSTR::null(), PCWSTR(name.as_ptr()), &mut luid)
            .map_err(|e| format!("查找权限失败: {}", e))?;

        let mut tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let _ = AdjustTokenPrivileges(
            token,
            false,
            Some(&mut tp),
            0,
            None,
            None,
        );
        let _ = CloseHandle(token);
    }
    Ok(())
}

#[cfg(windows)]
fn purge_via_nt() -> Result<(), String> {
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
    // MemoryPurgeLowPriorityStandbyList — 多数环境无需管理员
    const MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST: i32 = 5;
    const MEMORY_PURGE_STANDBY_LIST: i32 = 4;

    #[repr(C)]
    struct MemoryListCommand {
        command: i32,
        flags: u32,
    }

    for cmd in [MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST, MEMORY_PURGE_STANDBY_LIST] {
        let mut info = MemoryListCommand { command: cmd, flags: 0 };
        let status = unsafe {
            NtSetSystemInformation(
                SYSTEM_MEMORY_LIST_INFORMATION,
                (&mut info as *mut MemoryListCommand).cast(),
                std::mem::size_of::<MemoryListCommand>() as u32,
            )
        };
        if status == 0 {
            return Ok(());
        }
    }
    Err("释放失败：请右键「以管理员身份运行」LiteMonitor 后再试".into())
}
