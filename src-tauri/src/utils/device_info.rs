use once_cell::sync::Lazy;
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsType {
    Windows,
    Mac,
    Linux,
    Unknown,
}

pub static GLOBAL_OS_TYPE: Lazy<String> = Lazy::new(|| get_os_type_str().to_string());
pub static GLOBAL_DEVICE_ID: Lazy<String> = Lazy::new(|| get_device_id());

pub fn get_os_type() -> OsType {
    let os = env::consts::OS;
    match os {
        "windows" => OsType::Windows,
        "macos" => OsType::Mac,
        "linux" => OsType::Linux,
        _ => OsType::Unknown,
    }
}

pub fn get_os_type_str() -> &'static str {
    match get_os_type() {
        OsType::Windows => "windows",
        OsType::Mac => "macos",
        OsType::Linux => "linux",
        OsType::Unknown => "unknown",
    }
}

/// 获取设备唯一ID（优先主板序列号、MAC地址，否则用UUID）
pub fn get_device_id() -> String {
    #[cfg(target_os = "windows")]
    {
        use std::process::{Command, Stdio};
        use std::os::windows::process::CommandExt;
        
        // 尝试获取主板序列号 - 隐藏CMD窗口
        if let Ok(output) = Command::new("wmic")
            .args(["baseboard", "get", "serialnumber"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        {
            let out = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = out.lines().collect();
            if lines.len() > 1 {
                let sn = lines[1].trim();
                if !sn.is_empty() && sn != "To be filled by O.E.M." {
                    return sn.to_string();
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::{Command, Stdio};
        // 在macOS上隐藏终端窗口
        if let Ok(output) = Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        {
            let out = String::from_utf8_lossy(&output.stdout);
            for line in out.lines() {
                if line.contains("IOPlatformUUID") {
                    if let Some(uuid) = line.split('=').nth(1) {
                        return uuid.replace('"', "").trim().to_string();
                    }
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        // 尝试读取 /etc/machine-id
        if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
            let id = id.trim();
            if !id.is_empty() {
                return id.to_string();
            }
        }
        // 尝试读取 /var/lib/dbus/machine-id
        if let Ok(id) = std::fs::read_to_string("/var/lib/dbus/machine-id") {
            let id = id.trim();
            if !id.is_empty() {
                return id.to_string();
            }
        }
    }
    // 兜底
    String::new()
}
