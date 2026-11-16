//! 화면 목록 — CoreGraphics 전역 좌표(주 화면 왼쪽 위 원점 · 아래로 +y · point). Windows 의 모니터 좌표와 같은
//! 방향이라 코어의 가장자리 판정·캔버스 배치를 그대로 쓴다. 이름 = CGDirectDisplayID 10진(Swift 판 `displayID` 와 같은 값).

use super::ffi::{CGDisplayBounds, CGEventCreate, CGEventGetLocation, CGGetActiveDisplayList, CGMainDisplayID, CGPoint};
use core_foundation_sys::base::CFRelease;
use logiflow_core::edge::HandoffRect;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monitor {
    /// CGDirectDisplayID(10진).
    pub name: String,
    pub bounds: HandoffRect,
    pub primary: bool,
    pub id: u32,
}

pub fn all() -> Vec<Monitor> {
    let mut ids = [0u32; 32];
    let mut n: u32 = 0;
    if unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut n) } != 0 {
        return Vec::new();
    }
    let main = unsafe { CGMainDisplayID() };
    ids[..n as usize]
        .iter()
        .map(|&id| {
            let r = unsafe { CGDisplayBounds(id) };
            Monitor {
                name: id.to_string(),
                bounds: HandoffRect::new(
                    r.origin.x.round() as i32,
                    r.origin.y.round() as i32,
                    r.size.width.round() as i32,
                    r.size.height.round() as i32,
                ),
                primary: id == main,
                id,
            }
        })
        .collect()
}

pub fn names(list: &[Monitor]) -> Vec<String> {
    list.iter().map(|m| m.name.clone()).collect()
}

/// 지금 커서 위치(CG 전역 좌표, point).
pub fn cursor() -> Option<CGPoint> {
    unsafe {
        let ev = CGEventCreate(std::ptr::null());
        if ev.is_null() {
            return None;
        }
        let p = CGEventGetLocation(ev);
        CFRelease(ev as _);
        Some(p)
    }
}

/// 점이 올라간 화면(경계 1pt 여유) — 없으면 가장 가까운 화면.
pub fn from_point(list: &[Monitor], x: f64, y: f64) -> Option<&Monitor> {
    let inside = |m: &&Monitor| {
        let b = m.bounds;
        x >= b.left as f64 - 1.0 && x < (b.left + b.width) as f64 + 1.0 && y >= b.top as f64 - 1.0 && y < (b.top + b.height) as f64 + 1.0
    };
    if let Some(m) = list.iter().find(|m| {
        let b = m.bounds;
        x >= b.left as f64 && x < (b.left + b.width) as f64 && y >= b.top as f64 && y < (b.top + b.height) as f64
    }) {
        return Some(m);
    }
    if let Some(m) = list.iter().find(inside) {
        return Some(m);
    }
    list.iter().min_by(|a, b| dist(a, x, y).total_cmp(&dist(b, x, y)))
}

fn dist(m: &Monitor, x: f64, y: f64) -> f64 {
    let b = m.bounds;
    let cx = x.clamp(b.left as f64, (b.left + b.width) as f64);
    let cy = y.clamp(b.top as f64, (b.top + b.height) as f64);
    (cx - x).hypot(cy - y)
}

/// 주 화면 높이(Cocoa ↔ CG 좌표 바꾸기).
pub fn main_height() -> f64 {
    unsafe { CGDisplayBounds(CGMainDisplayID()).size.height }
}
