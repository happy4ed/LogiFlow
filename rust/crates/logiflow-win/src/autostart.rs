//! 로그인 시 트레이 자동 시작 — C# `AutoStart` 이식. 두 경로(사용자 범위·관리자 불필요):
//! ① 로그온 예약 작업(ONLOGON · LIMITED) — 로그인 직후 바로. ② HKCU Run — 작업이 정책으로 막힌 환경 보조.

use crate::paths::{RUN_VALUE, TRAY_TASK};
use crate::util::{exe_path, run_hidden, wide};
use windows::core::PCWSTR;
use windows::Win32::System::Registry::*;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn set_run(enable: bool) {
    let sk = wide(RUN_KEY);
    let nm = wide(RUN_VALUE);
    unsafe {
        let mut key = HKEY::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(sk.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_WRITE,
            None,
            &mut key,
            None,
        )
        .is_err()
        {
            return;
        }
        if enable {
            let v = wide(&format!("\"{}\"", exe_path().display()));
            let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2);
            let _ = RegSetValueExW(key, PCWSTR(nm.as_ptr()), None, REG_SZ, Some(bytes));
        } else {
            let _ = RegDeleteValueW(key, PCWSTR(nm.as_ptr()));
        }
        let _ = RegCloseKey(key);
    }
}

pub fn apply(enable: bool) {
    // ① HKCU Run (보조)
    set_run(enable);
    // ② 로그온 예약 작업(주) — 현재 사용자, 로그온 시, 비상승.
    if enable {
        let tr = format!("\\\"{}\\\"", exe_path().display());
        run_hidden(
            "schtasks.exe",
            &format!("/Create /TN \"{}\" /TR \"{}\" /SC ONLOGON /RL LIMITED /F", TRAY_TASK, tr),
            8000,
        );
    } else {
        run_hidden("schtasks.exe", &format!("/Delete /TN \"{}\" /F", TRAY_TASK), 8000);
    }
}

/// 제거 정리용 — 둘 다 지운다.
pub fn remove_all() {
    apply(false);
}
