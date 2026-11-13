//! raw 마우스 입력 — C# `RegisterMouseRawInput`·`TryReadRelativeMouse` 이식(LocalDex 출처).

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Input::*;

pub const WM_INPUT: u32 = 0x00FF;

/// 마우스(usage page 1 / usage 2) raw 입력 등록(RIDEV_INPUTSINK) 또는 해제.
pub fn register_mouse(window: HWND, add: bool) -> bool {
    let dev = RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x02,
        dwFlags: if add { RIDEV_INPUTSINK } else { RIDEV_REMOVE },
        hwndTarget: if add { window } else { HWND::default() },
    };
    unsafe { RegisterRawInputDevices(&[dev], std::mem::size_of::<RAWINPUTDEVICE>() as u32).is_ok() }
}

/// WM_INPUT 패킷에서 상대 이동(dx,dy)만. 절대 좌표 장치(태블릿·원격)는 무시.
pub fn read_relative_mouse(lparam: LPARAM) -> Option<(i32, i32)> {
    let header_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;
    let h = HRAWINPUT(lparam.0 as *mut _);
    let mut size = 0u32;
    unsafe {
        GetRawInputData(h, RID_INPUT, None, &mut size, header_size);
        if size == 0 || size > 1024 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if GetRawInputData(h, RID_INPUT, Some(buf.as_mut_ptr() as *mut _), &mut size, header_size) != size {
            return None;
        }
        let dw_type = u32::from_ne_bytes(buf[0..4].try_into().ok()?);
        if dw_type != RIM_TYPEMOUSE.0 {
            return None;
        }
        let off = header_size as usize;
        let flags = u16::from_ne_bytes(buf[off..off + 2].try_into().ok()?);
        if flags & 0x01 != 0 {
            return None; // MOUSE_MOVE_ABSOLUTE
        }
        let dx = i32::from_ne_bytes(buf[off + 12..off + 16].try_into().ok()?);
        let dy = i32::from_ne_bytes(buf[off + 16..off + 20].try_into().ok()?);
        Some((dx, dy))
    }
}
