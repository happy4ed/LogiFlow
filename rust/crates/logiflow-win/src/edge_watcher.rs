//! 가장자리 자동 전환 감시기 — C# `EdgeWatcher` 이식. 메시지 전용 창으로 WM_INPUT(raw 마우스)을 받아,
//! 커서가 배치된 모니터 변의 좁은 띠에서 dwell + 바깥 밀기를 만족하면 그 변의 채널로 전환을 트리거한다.
//!
//! 지금 판과 다른 점: 「드래그 중 전환 안 함」(drag_guard) 값을 실제로 따른다. 버튼 상태는
//! GetAsyncKeyState(실제 버튼)로 읽는다 — 백그라운드 스레드의 GetKeyState 는 갱신되지 않을 수 있다.

use crate::glow_win;
use crate::monitors;
use crate::rawinput::{self, WM_INPUT};
use crate::util::tick_ms;
use crate::wnd::{self, Handler};
use logiflow_core::config::EdgeAttachment;
use logiflow_core::edge::{EdgeDetector, HandoffEdge, HandoffRect, DEFAULT_BAND_PX};
use logiflow_core::glow::EdgeGlow;
use logiflow_core::log::DiagLogger;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

struct Attach {
    channel: i32,
    monitor: String,
    edge: HandoffEdge,
    bounds: HandoffRect,
    det: EdgeDetector,
}

struct Watcher {
    hwnd: HWND,
    log: Arc<DiagLogger>,
    on_trigger: Rc<dyn Fn(i32)>,
    attaches: Vec<Attach>,
    dwell_ms: i32,
    drag_guard: bool,
    running: bool,
    last_sample_ms: i64,
    last_glow_active: bool,
}

thread_local! {
    static W: RefCell<Option<Watcher>> = const { RefCell::new(None) };
}

/// UI 스레드에서 한 번. on_trigger(채널 0-base) 는 UI 스레드에서 불린다.
pub fn init(log: Arc<DiagLogger>, on_trigger: Rc<dyn Fn(i32)>) {
    let handler: Handler = Rc::new(|_h, msg, _w, l| {
        if msg == WM_INPUT {
            on_raw_input(l);
        }
        None // WM_INPUT 도 DefWindowProc 로 넘겨 정리.
    });
    if let Some(h) = wnd::message_window("LogiFlowRs.EdgeWatcher", handler) {
        W.with(|w| {
            *w.borrow_mut() = Some(Watcher {
                hwnd: h,
                log,
                on_trigger,
                attaches: Vec::new(),
                dwell_ms: logiflow_core::edge::DEFAULT_DWELL_MS,
                drag_guard: true,
                running: false,
                last_sample_ms: -1,
                last_glow_active: false,
            })
        });
    }
}

/// 배치를 현재 모니터 배치에 맞춰 다시 구성한다.
pub fn configure(dwell_ms: i32, drag_guard: bool, attachments: &[EdgeAttachment]) {
    let mons = monitors::all();
    W.with(|w| {
        if let Some(w) = w.borrow_mut().as_mut() {
            w.dwell_ms = dwell_ms;
            w.drag_guard = drag_guard;
            w.attaches.clear();
            for a in attachments {
                match mons.iter().find(|m| m.name == a.monitor_device_name) {
                    None => w.log.log(&format!("엣지 설정 건너뜀(모니터 없음): {}", a.monitor_device_name)),
                    Some(m) => w.attaches.push(Attach {
                        channel: a.channel,
                        monitor: a.monitor_device_name.clone(),
                        edge: a.edge,
                        bounds: m.bounds,
                        det: EdgeDetector::new(dwell_ms, logiflow_core::edge::DEFAULT_MIN_PUSH_PX),
                    }),
                }
            }
            w.log.log(&format!("엣지 감시 구성: 배치 {}개, dwell {}ms", w.attaches.len(), dwell_ms));
        }
    });
}

pub fn has_attachments() -> bool {
    W.with(|w| w.borrow().as_ref().map(|w| !w.attaches.is_empty()).unwrap_or(false))
}

pub fn is_running() -> bool {
    W.with(|w| w.borrow().as_ref().map(|w| w.running).unwrap_or(false))
}

/// 전환이 실패하면 변을 떠나지 않고 계속 밀어도 다시 시도되게 재무장(UI 스레드).
pub fn rearm() {
    W.with(|w| {
        if let Some(w) = w.borrow_mut().as_mut() {
            for a in &mut w.attaches {
                a.det.reset(false);
            }
        }
    });
}

pub fn start() {
    W.with(|w| {
        if let Some(w) = w.borrow_mut().as_mut() {
            if w.running {
                return;
            }
            if !rawinput::register_mouse(w.hwnd, true) {
                w.log.error("raw input 등록 실패 — 엣지 감시 비활성");
                return;
            }
            w.running = true;
            w.log.log("엣지 감시 시작");
        }
    });
}

pub fn stop() {
    let glow_off = W.with(|w| {
        if let Some(w) = w.borrow_mut().as_mut() {
            if !w.running {
                return false;
            }
            rawinput::register_mouse(w.hwnd, false);
            w.running = false;
            let was = w.last_glow_active;
            w.last_glow_active = false;
            w.log.log("엣지 감시 중지");
            return was;
        }
        false
    });
    if glow_off {
        glow_win::update(EdgeGlow::default());
    }
}

fn any_button_down() -> bool {
    unsafe {
        [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
            .iter()
            .any(|k| (GetAsyncKeyState(k.0 as i32) as u16 & 0x8000) != 0)
    }
}

enum Outcome {
    Nothing,
    Glow(EdgeGlow),
    Fire(EdgeGlow, i32, Rc<dyn Fn(i32)>),
}

fn on_raw_input(l: windows::Win32::Foundation::LPARAM) {
    let outcome = W.with(|w| {
        let mut borrow = w.borrow_mut();
        let w = match borrow.as_mut() {
            Some(w) if w.running && !w.attaches.is_empty() => w,
            _ => return Outcome::Nothing,
        };
        let (dx, dy) = match rawinput::read_relative_mouse(l) {
            Some(d) => d,
            None => return Outcome::Nothing,
        };
        let mut cur = POINT::default();
        if unsafe { GetCursorPos(&mut cur) }.is_err() {
            return Outcome::Nothing;
        }
        let now = tick_ms() as i64;
        let button_down = w.drag_guard && any_button_down();
        let dev = monitors::name_from_point(cur.x, cur.y);

        let mut glow_at: Option<usize> = None;
        let mut glow_progress = 0.0;
        let mut fired: Option<usize> = None;
        for i in 0..w.attaches.len() {
            let a = &mut w.attaches[i];
            if dev.as_deref() != Some(a.monitor.as_str()) {
                // 다른 모니터에 있으면 그 변의 dwell 은 리셋.
                a.det.reset(false);
                continue;
            }
            let push = EdgeDetector::outward_push(a.edge, dx, dy);
            // 샘플 로그(변 8px 안, 400ms 마다).
            let dist = EdgeDetector::distance_from_edge(&a.bounds, a.edge, cur.x, cur.y);
            if dist <= 8 && (w.last_sample_ms < 0 || now - w.last_sample_ms >= 400) {
                w.last_sample_ms = now;
                w.log.log(&format!(
                    "엣지 샘플: x={} y={} 변={} 채널={} 거리={} 푸시={} 진행={:.2}",
                    cur.x,
                    cur.y,
                    a.edge.name(),
                    a.channel + 1,
                    dist,
                    push,
                    a.det.progress(now)
                ));
            }
            let a = &mut w.attaches[i];
            if a.det.feed(&a.bounds, a.edge, cur.x, cur.y, push, now, false, button_down) {
                fired = Some(i);
                break;
            }
            // 전환 직후(재무장 전)엔 커서가 변에 남아 있어도 그리지 않는다.
            let dist = EdgeDetector::distance_from_edge(&a.bounds, a.edge, cur.x, cur.y);
            if !button_down && a.det.armed() && dist <= DEFAULT_BAND_PX {
                let p = a.det.progress(now);
                if glow_at.is_none() || p >= glow_progress {
                    glow_at = Some(i);
                    glow_progress = p;
                }
            }
        }
        if let Some(i) = fired {
            let (bounds, edge, channel, mon) = {
                let a = &w.attaches[i];
                (a.bounds, a.edge, a.channel, a.monitor.clone())
            };
            w.log
                .log(&format!("엣지 전환 트리거: 모니터 {} 변 {} → 채널 {}", mon, edge.name(), channel + 1));
            // 다른 변의 dwell 도 리셋(중복 트리거 방지).
            for o in &mut w.attaches {
                o.det.reset(true);
            }
            w.last_glow_active = false;
            let g = EdgeGlow {
                active: true,
                fired: true,
                monitor: bounds,
                edge: Some(edge),
                progress: 1.0,
                channel,
            };
            return Outcome::Fire(g, channel, w.on_trigger.clone());
        }
        if let Some(i) = glow_at {
            let a = &w.attaches[i];
            w.last_glow_active = true;
            return Outcome::Glow(EdgeGlow {
                active: true,
                fired: false,
                monitor: a.bounds,
                edge: Some(a.edge),
                progress: glow_progress,
                channel: a.channel,
            });
        }
        if w.last_glow_active {
            w.last_glow_active = false;
            return Outcome::Glow(EdgeGlow::default());
        }
        Outcome::Nothing
    });
    match outcome {
        Outcome::Nothing => {}
        Outcome::Glow(g) => glow_win::update(g),
        Outcome::Fire(g, ch, cb) => {
            // 전환 요청: glow 는 실제 전달(delivered)을 기다린다.
            glow_win::update(g);
            cb(ch);
        }
    }
}

pub fn dispose() {
    stop();
    W.with(|w| {
        if let Some(w) = w.borrow_mut().take() {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(w.hwnd);
            }
        }
    });
}
