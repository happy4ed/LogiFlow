//! 알림 영역(트레이) 아이콘 — WinForms `NotifyIcon` 대응(툴팁 · 풍선 · 탐색기 재시작 시 다시 등록).
//! 아이콘 리소스 번호 1 = logiflow.ico(.rc 에 내장).

use crate::util::wide;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WM_TRAY: u32 = WM_APP + 10;
const ICON_ID: u32 = 1;

pub fn app_icon(size: i32) -> HICON {
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        // MAKEINTRESOURCE(1)
        match LoadImageW(
            Some(hinst.into()),
            PCWSTR(std::ptr::without_provenance(1)),
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        ) {
            Ok(h) => HICON(h.0),
            Err(_) => LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
        }
    }
}

fn copy_into(dst: &mut [u16], s: &str) {
    let w = wide(s);
    let n = w.len().min(dst.len());
    dst[..n].copy_from_slice(&w[..n]);
    if let Some(last) = dst.last_mut() {
        *last = 0;
    }
}

fn base(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        ..Default::default()
    }
}

pub fn add(hwnd: HWND, tip: &str) {
    let mut d = base(hwnd);
    d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    d.uCallbackMessage = WM_TRAY;
    d.hIcon = app_icon(unsafe { GetSystemMetrics(SM_CXSMICON) });
    copy_into(&mut d.szTip, tip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &d);
    }
}

pub fn set_tip(hwnd: HWND, tip: &str) {
    let mut d = base(hwnd);
    d.uFlags = NIF_TIP;
    copy_into(&mut d.szTip, tip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &d);
    }
}

/// 풍선 알림(정보·경고). WinForms ShowBalloonTip 과 같은 NIF_INFO.
pub fn balloon(hwnd: HWND, title: &str, text: &str, warning: bool) {
    let mut d = base(hwnd);
    d.uFlags = NIF_INFO;
    copy_into(&mut d.szInfoTitle, title);
    copy_into(&mut d.szInfo, text);
    d.dwInfoFlags = if warning { NIIF_WARNING } else { NIIF_INFO };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &d);
    }
}

pub fn remove(hwnd: HWND) {
    let d = base(hwnd);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &d);
    }
}

/// 탐색기 재시작 알림 메시지(이걸 받으면 아이콘을 다시 add).
pub fn taskbar_created_msg() -> u32 {
    let w = wide("TaskbarCreated");
    unsafe { RegisterWindowMessageW(PCWSTR(w.as_ptr())) }
}

// ---- 팝업 메뉴 ----

pub enum MenuItem {
    Item { id: u32, text: String, enabled: bool },
    Separator,
}

/// 화면 좌표(x,y)에 메뉴를 띄우고 고른 id(없으면 0). owner 창을 전경으로 올려 메뉴가 바로 닫히게.
pub fn popup_menu(owner: HWND, items: &[MenuItem], x: i32, y: i32, align_top_left: bool) -> u32 {
    unsafe {
        let menu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return 0,
        };
        for it in items {
            match it {
                MenuItem::Separator => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                }
                MenuItem::Item { id, text, enabled } => {
                    let w = wide(text);
                    let flags = if *enabled { MF_STRING } else { MF_STRING | MF_GRAYED };
                    let _ = AppendMenuW(menu, flags, *id as usize, PCWSTR(w.as_ptr()));
                }
            }
        }
        let _ = SetForegroundWindow(owner);
        let align = if align_top_left {
            TPM_LEFTALIGN | TPM_TOPALIGN
        } else {
            TPM_RIGHTALIGN | TPM_BOTTOMALIGN
        };
        let cmd = TrackPopupMenuEx(menu, (TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | align).0, x, y, owner, None);
        let _ = PostMessageW(
            Some(owner),
            WM_NULL,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
        let _ = DestroyMenu(menu);
        cmd.0 as u32
    }
}
