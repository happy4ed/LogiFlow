//! 시스템 다크 모드 — C# `Theme.SystemIsDark`(HKCU Personalize `AppsUseLightTheme` = 0 이면 다크).

use crate::util::wide;
use windows::core::PCWSTR;
use windows::Win32::System::Registry::*;

pub fn read_dword(root: HKEY, subkey: &str, name: &str) -> Option<u32> {
    let sk = wide(subkey);
    let nm = wide(name);
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(root, PCWSTR(sk.as_ptr()), Some(0), KEY_READ, &mut key).is_err() {
            return None;
        }
        let mut data = 0u32;
        let mut size = 4u32;
        let mut ty = REG_VALUE_TYPE::default();
        let r = RegQueryValueExW(
            key,
            PCWSTR(nm.as_ptr()),
            None,
            Some(&mut ty),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if r.is_ok() && ty == REG_DWORD {
            Some(data)
        } else {
            None
        }
    }
}

pub fn system_is_dark() -> bool {
    read_dword(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "AppsUseLightTheme",
    ) == Some(0)
}

pub fn is_dark(mode: logiflow_core::config::ThemeMode) -> bool {
    use logiflow_core::config::ThemeMode::*;
    match mode {
        Light => false,
        Dark => true,
        System => system_is_dark(),
    }
}
