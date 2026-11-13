//! 모니터 열거 — C# `Screen.AllScreens`(EnumDisplayMonitors 순서)·`Screen.FromPoint` 대응.
//! 좌표계는 이 프로세스의 DPI 인식 상태를 따른다(상주 프로세스 = DPI 미인식 = 지금 판과 같은 논리 픽셀).

use crate::util::from_wide;
use logiflow_core::edge::HandoffRect;
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monitor {
    /// 장치 이름(예: `\\.\DISPLAY1`).
    pub name: String,
    pub bounds: HandoffRect,
    pub primary: bool,
}

unsafe extern "system" fn enum_cb(hmon: HMONITOR, _hdc: HDC, _rc: *mut RECT, data: LPARAM) -> BOOL {
    unsafe {
        let list = &mut *(data.0 as *mut Vec<Monitor>);
        if let Some(m) = info(hmon) {
            list.push(m);
        }
    }
    BOOL(1)
}

fn info(hmon: HMONITOR) -> Option<Monitor> {
    unsafe {
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(hmon, &mut mi as *mut _ as *mut MONITORINFO).as_bool() {
            return None;
        }
        let r = mi.monitorInfo.rcMonitor;
        Some(Monitor {
            name: from_wide(&mi.szDevice),
            bounds: HandoffRect::new(r.left, r.top, r.right - r.left, r.bottom - r.top),
            primary: (mi.monitorInfo.dwFlags & 1) != 0, // MONITORINFOF_PRIMARY
        })
    }
}

pub fn all() -> Vec<Monitor> {
    let mut list: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(enum_cb), LPARAM(&mut list as *mut _ as isize));
    }
    list
}

pub fn names(list: &[Monitor]) -> Vec<String> {
    list.iter().map(|m| m.name.clone()).collect()
}

/// 커서가 올라간 모니터 이름(C# Screen.FromPoint — 가장 가까운 모니터).
pub fn name_from_point(x: i32, y: i32) -> Option<String> {
    unsafe {
        let h = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        info(h).map(|m| m.name)
    }
}
