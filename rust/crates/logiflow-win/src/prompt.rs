//! 한 줄 입력 대화상자 — C# `Prompt.Show`(340×130, Windows 기본 입력란·[확인]/[취소]) 대응.
//! Windows 기본 컨트롤이라 한글 IME 가 그대로 된다. 설정 창(배율 인식 프로세스)에서 쓰므로 DPI 에 맞춰 키운다.

use crate::util::wide;
use crate::wnd::{self, Handler};
use logiflow_core::theme::Rgb;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::{EM_SETPASSWORDCHAR, EM_SETSEL};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::*;

const IDOK_: i32 = 1;
const IDCANCEL_: i32 = 2;
const ID_EDIT: i32 = 100;
const ID_EYE: i32 = 101;
/// Segoe MDL2 Assets 「보기」·「숨기기」 그림 글자(Windows 10·11 기본 글꼴).
const GLYPH_VIEW: &str = "\u{E890}";
const GLYPH_HIDE: &str = "\u{ED1A}";
const MASK: u16 = 0x25CF; // ●
const DM_GETDEFID: u32 = WM_USER;
const DC_HASDEFID: u32 = 0x534B;

fn colorref(c: Rgb) -> COLORREF {
    COLORREF(c.0 as u32 | (c.1 as u32) << 8 | (c.2 as u32) << 16)
}

/// 확인 = Some(앞뒤 공백 제거한 값), 취소 = None.
pub fn show(owner: HWND, title: &str, label: &str, initial: &str, bg: Rgb, fg: Rgb) -> Option<String> {
    show_inner(owner, title, label, initial, bg, fg, false)
}

/// 가린 입력(암호문구) — 글자는 ●로 가리고, 오른쪽 눈 아이콘 단추로 보기/숨기기(오타 확인용).
pub fn show_secret(owner: HWND, title: &str, label: &str, initial: &str, bg: Rgb, fg: Rgb) -> Option<String> {
    show_inner(owner, title, label, initial, bg, fg, true)
}

fn show_inner(owner: HWND, title: &str, label: &str, initial: &str, bg: Rgb, fg: Rgb, secret: bool) -> Option<String> {
    let dpi = unsafe { GetDpiForWindow(owner) }.max(96) as i32;
    let s = |v: i32| v * dpi / 96;
    let done: Rc<Cell<Option<bool>>> = Rc::new(Cell::new(None));
    let edit_hwnd: Rc<Cell<HWND>> = Rc::new(Cell::new(HWND::default()));
    let result: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let brush = unsafe { CreateSolidBrush(colorref(bg)) };

    let shown = Rc::new(Cell::new(false));
    let handler: Handler = {
        let (done, edit_hwnd, result, shown) = (done.clone(), edit_hwnd.clone(), result.clone(), shown.clone());
        Rc::new(move |h, msg, wp, lp| match msg {
            WM_COMMAND => {
                let id = (wp.0 & 0xFFFF) as i32;
                if id == ID_EYE {
                    let e = edit_hwnd.get();
                    let now = !shown.get();
                    shown.set(now);
                    unsafe {
                        SendMessageW(e, EM_SETPASSWORDCHAR, Some(WPARAM(if now { 0 } else { MASK as usize })), Some(LPARAM(0)));
                        let _ = InvalidateRect(Some(e), None, true);
                        if let Ok(b) = GetDlgItem(Some(h), ID_EYE) {
                            let t = wide(if now { GLYPH_HIDE } else { GLYPH_VIEW });
                            let _ = SetWindowTextW(b, PCWSTR(t.as_ptr()));
                        }
                        let _ = SetFocus(Some(e));
                    }
                    Some(LRESULT(0))
                } else if id == IDOK_ {
                    let e = edit_hwnd.get();
                    let len = unsafe { GetWindowTextLengthW(e) } as usize;
                    let mut buf = vec![0u16; len + 1];
                    unsafe { GetWindowTextW(e, &mut buf) };
                    *result.borrow_mut() = crate::util::from_wide(&buf);
                    done.set(Some(true));
                    Some(LRESULT(0))
                } else if id == IDCANCEL_ {
                    done.set(Some(false));
                    Some(LRESULT(0))
                } else {
                    None
                }
            }
            WM_CLOSE => {
                done.set(Some(false));
                Some(LRESULT(0))
            }
            DM_GETDEFID => Some(LRESULT(((DC_HASDEFID << 16) | IDOK_ as u32) as isize)),
            WM_CTLCOLORSTATIC => unsafe {
                let hdc = HDC(wp.0 as *mut _);
                SetTextColor(hdc, colorref(fg));
                SetBkColor(hdc, colorref(bg));
                let _ = lp;
                Some(LRESULT(brush.0 as isize))
            },
            WM_ERASEBKGND => unsafe {
                let hdc = HDC(wp.0 as *mut _);
                let mut rc = RECT::default();
                let _ = GetClientRect(h, &mut rc);
                FillRect(hdc, &rc, brush);
                Some(LRESULT(1))
            },
            _ => None,
        })
    };

    // 창 크기: 클라이언트 340×130 이 되게.
    let style = WS_CAPTION | WS_SYSMENU | WS_POPUP | WINDOW_STYLE(DS_MODALFRAME as u32);
    let ex = WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT;
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: s(340),
        bottom: s(130),
    };
    unsafe {
        let _ = AdjustWindowRectExForDpi(&mut rc, style, false, ex, dpi as u32);
    }
    let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
    // 부모 가운데.
    let mut pr = RECT::default();
    unsafe {
        let _ = GetWindowRect(owner, &mut pr);
    }
    let (x, y) = (pr.left + ((pr.right - pr.left) - w) / 2, pr.top + ((pr.bottom - pr.top) - h) / 2);
    let dlg = wnd::create("LogiFlowRs.Prompt", title, ex, style, (x, y, w, h), Some(owner), handler)?;

    // 글꼴: Windows 기본 GUI 글꼴(WinForms 기본 글꼴과 같은 출처)을 DPI 만큼 키움.
    let font = unsafe {
        let mut lf = LOGFONTW::default();
        let stock = GetStockObject(DEFAULT_GUI_FONT);
        GetObjectW(stock, std::mem::size_of::<LOGFONTW>() as i32, Some(&mut lf as *mut _ as *mut _));
        lf.lfHeight = lf.lfHeight * dpi / 96;
        CreateFontIndirectW(&lf)
    };
    let child = |cls: &str, text: &str, style: WINDOW_STYLE, ex: WINDOW_EX_STYLE, id: i32, r: (i32, i32, i32, i32)| -> HWND {
        let c = wide(cls);
        let t = wide(text);
        unsafe {
            let h = CreateWindowExW(
                ex,
                PCWSTR(c.as_ptr()),
                PCWSTR(t.as_ptr()),
                WS_CHILD | WS_VISIBLE | style,
                s(r.0),
                s(r.1),
                s(r.2),
                s(r.3),
                Some(dlg),
                Some(HMENU(id as isize as *mut _)),
                None,
                None,
            )
            .unwrap_or_default();
            SendMessageW(h, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));
            h
        }
    };
    child("STATIC", label, WINDOW_STYLE(0), WINDOW_EX_STYLE(0), -1, (16, 16, 308, 20));
    let edit_style = WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32) | if secret { WINDOW_STYLE(ES_PASSWORD as u32) } else { WINDOW_STYLE(0) };
    let edit = child(
        "EDIT",
        initial,
        edit_style,
        WS_EX_CLIENTEDGE,
        ID_EDIT,
        (16, 42, if secret { 274 } else { 308 }, 20),
    );
    edit_hwnd.set(edit);
    let mut icon_font = HFONT::default();
    if secret {
        unsafe {
            SendMessageW(edit, EM_SETPASSWORDCHAR, Some(WPARAM(MASK as usize)), Some(LPARAM(0)));
        }
        let eye = child(
            "BUTTON",
            GLYPH_VIEW,
            WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            WINDOW_EX_STYLE(0),
            ID_EYE,
            (294, 41, 30, 22),
        );
        icon_font = unsafe {
            let mut lf = LOGFONTW {
                lfHeight: -(12 * dpi / 96),
                lfCharSet: DEFAULT_CHARSET,
                ..Default::default()
            };
            let face = wide("Segoe MDL2 Assets");
            for (i, ch) in face.iter().take(31).enumerate() {
                lf.lfFaceName[i] = *ch;
            }
            CreateFontIndirectW(&lf)
        };
        unsafe {
            SendMessageW(eye, WM_SETFONT, Some(WPARAM(icon_font.0 as usize)), Some(LPARAM(1)));
        }
    }
    child(
        "BUTTON",
        "확인",
        WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
        WINDOW_EX_STYLE(0),
        IDOK_,
        (160, 82, 76, 23),
    );
    child(
        "BUTTON",
        "취소",
        WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
        WINDOW_EX_STYLE(0),
        IDCANCEL_,
        (248, 82, 76, 23),
    );

    unsafe {
        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(dlg, SW_SHOW);
        SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
        let _ = SetFocus(Some(edit));
        let mut msg = MSG::default();
        while done.get().is_none() {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            if !IsDialogMessageW(dlg, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let _ = EnableWindow(owner, true);
        let _ = SetForegroundWindow(owner);
        let _ = DestroyWindow(dlg);
        let _ = DeleteObject(font.into());
        if !icon_font.is_invalid() {
            let _ = DeleteObject(icon_font.into());
        }
        let _ = DeleteObject(brush.into());
    }
    if done.get() == Some(true) {
        Some(result.borrow().trim().to_string())
    } else {
        None
    }
}
