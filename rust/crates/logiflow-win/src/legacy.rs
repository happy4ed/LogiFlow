//! C# 판과 나란히 — 감지(중복 전환 막기)와 첫 실행 설정 이어받기(계획서 6절 4·5).
//! C# 판 파일·설정은 읽기만 하고 절대 쓰지 않는다.

use crate::dpapi;
use crate::paths::{self, legacy};
use crate::service_ctl;
use crate::util::wide;
use logiflow_core::config::AppConfig;
use logiflow_core::log::DiagLogger;
use windows::core::PCWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};

/// C# 트레이가 이 세션에서 돌고 있나(단일 인스턴스 뮤텍스 존재).
pub fn csharp_tray_running() -> bool {
    let n = wide(legacy::MUTEX_NAME);
    unsafe {
        match OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(n.as_ptr())) {
            Ok(h) => {
                let _ = CloseHandle(h);
                true
            }
            Err(_) => false,
        }
    }
}

pub fn csharp_service_exists() -> bool {
    service_ctl::legacy_exists()
}

/// 가장자리 감시를 켜면 C# 판과 중복 전환이 날 수 있는가.
pub fn conflict() -> bool {
    csharp_tray_running() || csharp_service_exists()
}

/// 첫 실행이면 C# 판 config.txt·devices.txt 를 옮겨 온다. 시험판 한정: preLogon=0 · autostart=0.
/// 암호문구는 DPAPI 로 옮겨 담고, C# 원본 평문 파일은 건드리지 않는다.
pub fn import_if_first_run(log: &DiagLogger) -> bool {
    let ours = paths::config_path();
    if ours.exists() {
        return false;
    }
    let theirs = paths::legacy_user_dir().join("config.txt");
    if !theirs.exists() {
        return false;
    }
    let mut cfg = AppConfig::load(&theirs, None);
    cfg.pre_logon = false;
    cfg.auto_start = false;
    let saved = dpapi::with_codec(|c| cfg.save(&ours, Some(c))).is_ok();
    let dev_src = paths::legacy_user_dir().join("devices.txt");
    let dev_dst = paths::devices_path();
    if dev_src.exists() && !dev_dst.exists() {
        let _ = std::fs::copy(&dev_src, &dev_dst);
    }
    log.log(&format!(
        "C# 판 설정 이어받기: {} → {} ({}) · preLogon=0 · autostart=0",
        theirs.display(),
        ours.display(),
        if saved { "성공" } else { "실패" }
    ));
    saved
}
