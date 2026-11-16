//! 가장자리 glow/tension 창 — Windows `glow_win` 의 Mac 판. 상태·밝기 계산은 코어 `GlowState`, 색 띠 한 줄은 코어
//! `canonical_row`(Windows 와 같은 그림). 창은 테두리 없음 · 투명 · 클릭 통과 · 모든 데스크톱(Space)에 · 화면 보호기 층.
//! 띠 한 줄(90×1)을 이미지로 만들어 창 크기로 늘린다(길이 방향은 같은 색이라 늘려도 같다). 메인 스레드 전용 —
//! 다른 스레드의 `delivered` · `failed` 는 메인 큐로 넘긴다.

use super::mainq;
use super::monitors;
use super::theme_sys;
use super::util::tick_ms;
use logiflow_core::config::ThemeMode;
use logiflow_core::edge::{HandoffEdge, HandoffRect};
use logiflow_core::glow::{canonical_row, EdgeGlow, GlowAction, GlowState, MAX_BAND, TICK_MS};
use logiflow_core::theme::{Palette, Rgb};
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSImage, NSImageScaling, NSImageView, NSScreenSaverWindowLevel, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::CFData;
use objc2_core_graphics::{CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::cell::RefCell;

struct Glow {
    window: Retained<NSWindow>,
    view: Retained<NSImageView>,
    state: GlowState,
    theme: ThemeMode,
    timer: Option<core_foundation_sys::runloop::CFRunLoopTimerRef>,
    frame: NSRect,
}

thread_local! {
    static G: RefCell<Option<Glow>> = const { RefCell::new(None) };
}

fn accent(theme: ThemeMode) -> Rgb {
    Palette::of(theme_sys::is_dark(theme)).accent
}

/// 메인 스레드에서 한 번.
pub fn init(theme: ThemeMode) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let rect = NSRect::new(NSPoint::new(-10000.0, -10000.0), NSSize::new(1.0, 1.0));
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(NSWindow::alloc(mtm), rect, NSWindowStyleMask::Borderless, NSBackingStoreType::Buffered, false)
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setHasShadow(false);
    window.setIgnoresMouseEvents(true);
    window.setLevel(NSScreenSaverWindowLevel);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    let view = NSImageView::initWithFrame(NSImageView::alloc(mtm), rect);
    view.setImageScaling(NSImageScaling::ScaleAxesIndependently);
    window.setContentView(Some(&view));
    G.with(|g| {
        *g.borrow_mut() = Some(Glow {
            window,
            view,
            state: GlowState::default(),
            theme,
            timer: None,
            frame: rect,
        })
    });
}

pub fn set_theme(theme: ThemeMode) {
    G.with(|g| {
        if let Some(g) = g.borrow_mut().as_mut() {
            g.theme = theme;
        }
    });
}

/// 감시기 갱신(메인 스레드).
pub fn update(e: EdgeGlow) {
    let start = G.with(|g| {
        let mut b = g.borrow_mut();
        let Some(g) = b.as_mut() else {
            return false;
        };
        g.state.update(&e, tick_ms());
        g.state.timer_running && g.timer.is_none()
    });
    if start {
        start_timer();
    }
}

/// setHost 가 실제로 나간 순간(아무 스레드에서나).
pub fn delivered() {
    if !mainq::is_main() {
        mainq::post(delivered);
        return;
    }
    let act = G.with(|g| g.borrow_mut().as_mut().and_then(|g| g.state.delivered()));
    if let Some(GlowAction::Render { progress, peak }) = act {
        render(progress, peak);
    }
    start_timer();
}

/// 전환 실패·건너뜀(아무 스레드에서나). 최고점 없이 흐려진다.
pub fn failed() {
    if !mainq::is_main() {
        mainq::post(failed);
        return;
    }
    G.with(|g| {
        if let Some(g) = g.borrow_mut().as_mut() {
            g.state.failed();
        }
    });
    start_timer();
}

extern "C" fn on_tick(_t: core_foundation_sys::runloop::CFRunLoopTimerRef, _i: *mut std::ffi::c_void) {
    let act = G.with(|g| g.borrow_mut().as_mut().map(|g| g.state.tick(tick_ms())));
    match act {
        Some(GlowAction::Render { progress, peak }) => render(progress, peak),
        Some(GlowAction::HideAndStop) => {
            hide();
            stop_timer();
        }
        _ => {}
    }
}

fn start_timer() {
    use core_foundation_sys::runloop::*;
    G.with(|g| {
        let mut b = g.borrow_mut();
        let Some(g) = b.as_mut() else {
            return;
        };
        if g.timer.is_some() {
            return;
        }
        unsafe {
            let iv = TICK_MS as f64 / 1000.0;
            let t = CFRunLoopTimerCreate(
                core_foundation_sys::base::kCFAllocatorDefault,
                core_foundation_sys::date::CFAbsoluteTimeGetCurrent() + iv,
                iv,
                0,
                0,
                on_tick,
                std::ptr::null_mut(),
            );
            // 메뉴가 열려 있어도(이벤트 추적 모드) 움직이게 공통 모드에.
            CFRunLoopAddTimer(CFRunLoopGetMain(), t, kCFRunLoopCommonModes);
            g.timer = Some(t);
        }
    });
}

fn stop_timer() {
    G.with(|g| {
        if let Some(g) = g.borrow_mut().as_mut() {
            if let Some(t) = g.timer.take() {
                unsafe {
                    core_foundation_sys::runloop::CFRunLoopTimerInvalidate(t);
                    core_foundation_sys::base::CFRelease(t as _);
                }
            }
        }
    });
}

fn hide() {
    G.with(|g| {
        if let Some(g) = g.borrow().as_ref() {
            g.window.orderOut(None);
        }
    });
}

/// 띠 한 줄 이미지(변 방향대로 가로 90×1 또는 세로 1×90).
fn strip_image(edge: HandoffEdge, progress: f64, peak: bool, accent: Rgb) -> Option<Retained<NSImage>> {
    let row = canonical_row(progress, peak, accent); // x = MAX_BAND-1 이 변
    let n = MAX_BAND as usize;
    let px: Vec<u32> = match edge {
        HandoffEdge::Right | HandoffEdge::Bottom => row,
        HandoffEdge::Left | HandoffEdge::Top => row.into_iter().rev().collect(),
    };
    let (w, h) = match edge {
        HandoffEdge::Left | HandoffEdge::Right => (n, 1usize),
        HandoffEdge::Top | HandoffEdge::Bottom => (1usize, n),
    };
    // 0xAARRGGBB 를 리틀 엔디언으로 = B G R A 바이트(프리멀티플라이드).
    let bytes: Vec<u8> = px.iter().flat_map(|p| p.to_le_bytes()).collect();
    let data = CFData::from_bytes(&bytes);
    let provider = CGDataProvider::with_cf_data(Some(&data))?;
    let space = CGColorSpace::new_device_rgb()?;
    let info = CGBitmapInfo(CGImageByteOrderInfo::Order32Little.0 | CGImageAlphaInfo::PremultipliedFirst.0);
    let img = unsafe {
        CGImage::new(
            w,
            h,
            8,
            32,
            w * 4,
            Some(&space),
            info,
            Some(&provider),
            std::ptr::null(),
            true,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }?;
    Some(NSImage::initWithCGImage_size(NSImage::alloc(), &img, NSSize::new(w as f64, h as f64)))
}

/// 띠 창 위치(Cocoa 좌표 — 주 화면 왼쪽 아래 원점). mon 은 CG 좌표(왼쪽 위 원점).
fn band_frame(mon: &HandoffRect, edge: HandoffEdge) -> NSRect {
    let b = MAX_BAND as f64;
    let (l, t, w, h) = (mon.left as f64, mon.top as f64, mon.width as f64, mon.height as f64);
    let (x, y_top, bw, bh) = match edge {
        HandoffEdge::Right => (l + w - b, t, b, h),
        HandoffEdge::Left => (l, t, b, h),
        HandoffEdge::Top => (l, t, w, b),
        HandoffEdge::Bottom => (l, t + h - b, w, b),
    };
    let y = monitors::main_height() - (y_top + bh);
    NSRect::new(NSPoint::new(x, y), NSSize::new(bw, bh))
}

fn render(progress: f64, peak: bool) {
    G.with(|g| {
        let mut b = g.borrow_mut();
        let Some(g) = b.as_mut() else {
            return;
        };
        let Some(edge) = g.state.edge else {
            return;
        };
        let r = band_frame(&g.state.mon, edge);
        if r != g.frame {
            g.window.setFrame_display(r, false);
            g.view.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), r.size));
            g.frame = r;
        }
        if let Some(img) = strip_image(edge, progress, peak, accent(g.theme)) {
            g.view.setImage(Some(&img));
        }
        if !g.window.isVisible() {
            g.window.orderFrontRegardless();
        }
    });
}

pub fn dispose() {
    stop_timer();
    hide();
    G.with(|g| *g.borrow_mut() = None);
}
