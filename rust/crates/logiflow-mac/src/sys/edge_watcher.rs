//! 가장자리 자동 전환 감시기 — Windows `edge_watcher` 의 Mac 판(Swift 판 `EdgeWatcher` 와 같은 입력 경로).
//!
//! NSEvent 전역·로컬 모니터로 마우스 이동(끌기 포함)의 델타 = 실제 손 움직임을 받는다 — 커서가 화면 끝에 막혀
//! 있어도 「바깥으로 미는」 것을 잡는다. 마우스 이벤트 모니터는 손쉬운 사용·입력 모니터링 권한이 필요 없다.
//! 판정은 코어 `EdgeDetector`(Windows 와 같은 규칙: 좁은 띠 + dwell + 바깥 밀기 누적 · 드래그 막기 · 재무장).
//! 좌표 = CoreGraphics 전역 point(왼쪽 위 원점, 아래로 +y — NSEvent 델타도 아래로 +). 모든 처리는 메인 스레드.

use super::glow_mac;
use super::monitors::{self, Monitor};
use super::util::tick_ms;
use block2::RcBlock;
use logiflow_core::config::EdgeAttachment;
use logiflow_core::edge::{EdgeDetector, HandoffEdge, HandoffRect, DEFAULT_BAND_PX};
use logiflow_core::glow::EdgeGlow;
use logiflow_core::log::DiagLogger;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask};
use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;

struct Attach {
    channel: i32,
    monitor: String,
    edge: HandoffEdge,
    bounds: HandoffRect,
    det: EdgeDetector,
}

struct Watcher {
    log: Arc<DiagLogger>,
    on_trigger: Rc<dyn Fn(i32)>,
    attaches: Vec<Attach>,
    monitors: Vec<Monitor>,
    drag_guard: bool,
    running: bool,
    global: Option<Retained<AnyObject>>,
    local: Option<Retained<AnyObject>>,
    last_sample_ms: i64,
    last_glow_active: bool,
    /// 델타 소수점 이하를 모아 정수 픽셀 밀기로(트랙패드 미세 이동도 쌓이게).
    carry: (f64, f64),
}

thread_local! {
    static W: RefCell<Option<Watcher>> = const { RefCell::new(None) };
}

/// 메인 스레드에서 한 번. on_trigger(채널 0-base)는 메인 스레드에서 불린다.
pub fn init(log: Arc<DiagLogger>, on_trigger: Rc<dyn Fn(i32)>) {
    W.with(|w| {
        *w.borrow_mut() = Some(Watcher {
            log,
            on_trigger,
            attaches: Vec::new(),
            monitors: Vec::new(),
            drag_guard: true,
            running: false,
            global: None,
            local: None,
            last_sample_ms: -1,
            last_glow_active: false,
            carry: (0.0, 0.0),
        })
    });
}

/// 배치를 지금 화면 배치에 맞춰 다시 구성한다.
pub fn configure(dwell_ms: i32, drag_guard: bool, attachments: &[EdgeAttachment]) {
    let mons = monitors::all();
    W.with(|w| {
        if let Some(w) = w.borrow_mut().as_mut() {
            w.drag_guard = drag_guard;
            w.attaches.clear();
            for a in attachments {
                match mons.iter().find(|m| m.name == a.monitor_device_name) {
                    None => w.log.log(&format!("엣지 설정 건너뜀(화면 없음): {}", a.monitor_device_name)),
                    Some(m) => w.attaches.push(Attach {
                        channel: a.channel,
                        monitor: a.monitor_device_name.clone(),
                        edge: a.edge,
                        bounds: m.bounds,
                        det: EdgeDetector::new(dwell_ms, logiflow_core::edge::DEFAULT_MIN_PUSH_PX),
                    }),
                }
            }
            w.monitors = mons;
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

/// 전환이 실패하면 변을 떠나지 않고 계속 밀어도 다시 시도되게 재무장.
pub fn rearm() {
    W.with(|w| {
        if let Some(w) = w.borrow_mut().as_mut() {
            for a in &mut w.attaches {
                a.det.reset(false);
            }
        }
    });
}

fn mask() -> NSEventMask {
    NSEventMask::MouseMoved | NSEventMask::LeftMouseDragged | NSEventMask::RightMouseDragged | NSEventMask::OtherMouseDragged
}

pub fn start() {
    let go = W.with(|w| w.borrow().as_ref().map(|w| !w.running).unwrap_or(false));
    if !go {
        return;
    }
    // 모니터 등록(블록 안에서 W 를 다시 빌리므로 등록은 빌림 밖에서).
    let global = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
        mask(),
        &RcBlock::new(|ev: NonNull<NSEvent>| {
            on_event(unsafe { ev.as_ref() });
        }),
    );
    let local = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            mask(),
            &RcBlock::new(|ev: NonNull<NSEvent>| -> *mut NSEvent {
                on_event(ev.as_ref());
                ev.as_ptr()
            }),
        )
    };
    W.with(|w| {
        if let Some(w) = w.borrow_mut().as_mut() {
            if global.is_none() {
                w.log.error("마우스 이동 감시 등록 실패 — 엣지 감시 비활성");
            }
            w.global = global;
            w.local = local;
            w.running = true;
            w.carry = (0.0, 0.0);
            w.log.log("엣지 감시 시작");
        }
    });
}

pub fn stop() {
    let (glow_off, mons) = W.with(|w| {
        if let Some(w) = w.borrow_mut().as_mut() {
            if !w.running {
                return (false, Vec::new());
            }
            w.running = false;
            let mons: Vec<Retained<AnyObject>> = [w.global.take(), w.local.take()].into_iter().flatten().collect();
            let was = w.last_glow_active;
            w.last_glow_active = false;
            w.log.log("엣지 감시 중지");
            return (was, mons);
        }
        (false, Vec::new())
    });
    for m in mons {
        unsafe { NSEvent::removeMonitor(&m) };
    }
    if glow_off {
        glow_mac::update(EdgeGlow::default());
    }
}

/// 델타를 정수 픽셀로(소수점 이하는 다음 이벤트로 넘긴다).
fn quantize(carry: &mut f64, d: f64) -> i32 {
    let total = *carry + d;
    let whole = total.trunc();
    *carry = total - whole;
    whole as i32
}

enum Outcome {
    Nothing,
    Glow(EdgeGlow),
    Fire(EdgeGlow, i32, Rc<dyn Fn(i32)>),
}

fn on_event(ev: &NSEvent) {
    let outcome = W.with(|w| {
        let Ok(mut borrow) = w.try_borrow_mut() else {
            return Outcome::Nothing;
        };
        let w = match borrow.as_mut() {
            Some(w) if w.running && !w.attaches.is_empty() => w,
            _ => return Outcome::Nothing,
        };
        let dx = quantize(&mut w.carry.0, ev.deltaX());
        let dy = quantize(&mut w.carry.1, ev.deltaY());
        let Some(cur) = monitors::cursor() else {
            return Outcome::Nothing;
        };
        let now = tick_ms() as i64;
        let button_down = w.drag_guard && NSEvent::pressedMouseButtons() != 0;
        let dev = monitors::from_point(&w.monitors, cur.x, cur.y).map(|m| m.name.clone());
        // 오른쪽·아래 끝은 point 소수점 아래까지 오므로 내림(Windows 픽셀과 같은 [left, right) 범위).
        let (cx, cy) = (cur.x.floor() as i32, cur.y.floor() as i32);

        let mut glow_at: Option<usize> = None;
        let mut glow_progress = 0.0;
        let mut fired: Option<usize> = None;
        for i in 0..w.attaches.len() {
            let a = &mut w.attaches[i];
            if dev.as_deref() != Some(a.monitor.as_str()) {
                a.det.reset(false);
                continue;
            }
            let push = EdgeDetector::outward_push(a.edge, dx, dy);
            let dist = EdgeDetector::distance_from_edge(&a.bounds, a.edge, cx, cy);
            if dist <= 8 && (w.last_sample_ms < 0 || now - w.last_sample_ms >= 400) {
                w.last_sample_ms = now;
                w.log.log(&format!(
                    "엣지 샘플: x={:.1} y={:.1} 변={} 채널={} 거리={} 푸시={} 진행={:.2}",
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
            if a.det.feed(&a.bounds, a.edge, cx, cy, push, now, false, button_down) {
                fired = Some(i);
                break;
            }
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
            w.log.log(&format!("엣지 전환 트리거: 화면 {} 변 {} → 채널 {}", mon, edge.name(), channel + 1));
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
        Outcome::Glow(g) => glow_mac::update(g),
        Outcome::Fire(g, ch, cb) => {
            glow_mac::update(g);
            cb(ch);
        }
    }
}

pub fn dispose() {
    stop();
    W.with(|w| *w.borrow_mut() = None);
}
