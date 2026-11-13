//! glow 오버레이 창 — C# `GlowOverlayForm`(레이어드·클릭 통과·비활성·최상위, per-pixel alpha)과
//! `GlowController`(16ms 타이머) 이식. 상태 계산은 코어 `glow` 몫.
//!
//! 프로세스당 하나(트레이 또는 바탕화면 워커의 UI 스레드). 다른 스레드에서는 `delivered()`·`failed()`
//! 가 UI 스레드로 넘겨 실행한다.

use crate::theme_sys;
use crate::util::tick_ms;
use crate::wnd::{self, Handler};
use logiflow_core::config::ThemeMode;
use logiflow_core::glow::{self, EdgeGlow, GlowAction, GlowState, TICK_MS};
use logiflow_core::theme::Palette;
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::Foundation::{HWND, LRESULT, POINT, SIZE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const TIMER_ID: usize = 1;

struct GlowCtl {
    hwnd: HWND,
    state: GlowState,
    shown: bool,
    timer_on: bool,
    theme: ThemeMode,
}

thread_local! {
    static GLOW: RefCell<Option<GlowCtl>> = const { RefCell::new(None) };
}

/// UI 스레드에서 한 번. theme = 강조색 결정(시스템이면 레지스트리).
pub fn init(theme: ThemeMode) {
    let handler: Handler = Rc::new(|_h, msg, wp, _l| {
        if msg == WM_TIMER && wp.0 == TIMER_ID {
            on_tick();
            return Some(LRESULT(0));
        }
        None
    });
    let ex = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST;
    if let Some(h) = wnd::create("LogiFlowRs.Glow", "", ex, WS_POPUP, (-10000, -10000, 1, 1), None, handler) {
        GLOW.with(|g| {
            *g.borrow_mut() = Some(GlowCtl {
                hwnd: h,
                state: GlowState::default(),
                shown: false,
                timer_on: false,
                theme,
            })
        });
    }
}

pub fn set_theme(theme: ThemeMode) {
    GLOW.with(|g| {
        if let Some(c) = g.borrow_mut().as_mut() {
            c.theme = theme;
        }
    });
}

fn accent(theme: ThemeMode) -> logiflow_core::theme::Rgb {
    let dark = match theme {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => theme_sys::system_is_dark(),
    };
    Palette::of(dark).accent
}

fn ensure_timer(c: &mut GlowCtl) {
    if c.state.timer_running && !c.timer_on {
        unsafe {
            SetTimer(Some(c.hwnd), TIMER_ID, TICK_MS, None);
        }
        c.timer_on = true;
    } else if !c.state.timer_running && c.timer_on {
        unsafe {
            let _ = KillTimer(Some(c.hwnd), TIMER_ID);
        }
        c.timer_on = false;
    }
}

/// 감시기에서 UI 스레드로.
pub fn update(g: EdgeGlow) {
    GLOW.with(|cell| {
        if let Some(c) = cell.borrow_mut().as_mut() {
            c.state.update(&g, tick_ms());
            ensure_timer(c);
        }
    });
}

/// setHost 가 실제로 나간 순간(아무 스레드). 최고점을 바로 그리고 잠깐 뒤 숨긴다.
pub fn delivered() {
    wnd::post(|| {
        GLOW.with(|cell| {
            if let Some(c) = cell.borrow_mut().as_mut() {
                if let Some(GlowAction::Render { progress, peak }) = c.state.delivered() {
                    render(c, progress, peak);
                }
                ensure_timer(c);
            }
        })
    });
}

/// 전환 실패·건너뜀(아무 스레드). 최고점 없이 흐려진다.
pub fn failed() {
    wnd::post(|| {
        GLOW.with(|cell| {
            if let Some(c) = cell.borrow_mut().as_mut() {
                c.state.failed();
                ensure_timer(c);
            }
        })
    });
}

fn on_tick() {
    GLOW.with(|cell| {
        if let Some(c) = cell.borrow_mut().as_mut() {
            match c.state.tick(tick_ms()) {
                GlowAction::None => {}
                GlowAction::HideAndStop => hide(c),
                GlowAction::Render { progress, peak } => render(c, progress, peak),
            }
            ensure_timer(c);
        }
    });
}

fn hide(c: &mut GlowCtl) {
    if c.shown {
        unsafe {
            let _ = ShowWindow(c.hwnd, SW_HIDE);
        }
        c.shown = false;
    }
}

fn render(c: &mut GlowCtl, progress: f64, peak: bool) {
    let edge = match c.state.edge {
        Some(e) => e,
        None => return,
    };
    let bmp = match glow::render(&c.state.mon, edge, progress, peak, accent(c.theme)) {
        Some(b) => b,
        None => return,
    };
    update_layered(c.hwnd, bmp.width, bmp.height, &bmp.pixels, bmp.x, bmp.y);
    if !c.shown {
        unsafe {
            let _ = ShowWindow(c.hwnd, SW_SHOWNOACTIVATE);
        }
        c.shown = true;
    }
}

/// 프리멀티플라이드 BGRA 픽셀을 레이어드 창으로(UpdateLayeredWindow · ULW_ALPHA).
pub fn update_layered(hwnd: HWND, w: i32, h: i32, pixels: &[u32], x: i32, y: i32) {
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // 위→아래
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        if let Ok(dib) = CreateDIBSection(Some(mem), &bi, DIB_RGB_COLORS, &mut bits, None, 0) {
            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u32, (w * h) as usize);
            }
            let old = SelectObject(mem, dib.into());
            let size = SIZE { cx: w, cy: h };
            let pos = POINT { x, y };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                hwnd,
                Some(screen),
                Some(&pos),
                Some(&size),
                Some(mem),
                Some(&src),
                windows::Win32::Foundation::COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            SelectObject(mem, old);
            let _ = DeleteObject(dib.into());
        }
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
    }
}

pub fn dispose() {
    GLOW.with(|cell| {
        if let Some(c) = cell.borrow_mut().take() {
            unsafe {
                let _ = KillTimer(Some(c.hwnd), TIMER_ID);
                let _ = DestroyWindow(c.hwnd);
            }
        }
    });
}
