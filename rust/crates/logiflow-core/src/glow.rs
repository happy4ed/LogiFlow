//! 엣지 glow/tension 상태와 그림 계산 — C# `GlowController`(상태)·`RenderCanonical`(그림) 이식.
//! 창·타이머는 OS 어댑터 몫. 여기서는 16ms 틱마다 무엇을 그릴지와 픽셀을 계산한다.

use crate::edge::{HandoffEdge, HandoffRect};
use crate::theme::{blend, Rgb};

/// 엣지 glow 상태(감시기 → 컨트롤러). C# `EdgeGlow`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeGlow {
    /// 변에 닿아 progress 가 쌓이는 중.
    pub active: bool,
    /// 전환 요청 순간.
    pub fired: bool,
    pub monitor: HandoffRect,
    pub edge: Option<HandoffEdge>,
    /// 0..1
    pub progress: f64,
    pub channel: i32,
}

pub const MAX_BAND: i32 = 90;
/// dwell 동안의 상한 · 전환 요청 후 전달을 기다리는 동안의 밝기. 최고점(1.0)은 전달 순간 몫.
pub const DWELL_CEILING: f64 = 0.8;
pub const SENDING_LEVEL: f64 = 0.9;
/// 전달 신호를 이 시간 안에 못 받으면 실패로 보고 흐려진다.
pub const SEND_TIMEOUT_MS: u64 = 1500;
/// 전달 순간 최고점을 보이는 틱 수(16ms 단위).
pub const PEAK_TICKS: i32 = 3;
pub const TICK_MS: u32 = 16;

/// 한 번의 그리기 지시.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GlowAction {
    /// 아무것도 안 함(틱 계속).
    None,
    /// 이 밝기로 그림(peak = 흰빛 최고점).
    Render { progress: f64, peak: bool },
    /// 숨기고 타이머 멈춤.
    HideAndStop,
}

#[derive(Clone, Debug, Default)]
pub struct GlowState {
    has_target: bool,
    target_progress: f64,
    pub mon: HandoffRect,
    pub edge: Option<HandoffEdge>,
    shown: f64,
    sending: bool,
    send_start_ms: u64,
    peak_left: i32,
    /// 타이머가 돌아야 하는지(어댑터가 확인).
    pub timer_running: bool,
}

impl GlowState {
    /// 감시기 갱신(UI 스레드). 타이머 시작이 필요하면 timer_running=true 로 둔다.
    pub fn update(&mut self, g: &EdgeGlow, now_ms: u64) {
        if g.fired {
            self.mon = g.monitor;
            self.edge = g.edge;
            self.sending = true;
            self.send_start_ms = now_ms;
            self.peak_left = 0;
            self.has_target = true;
            self.target_progress = SENDING_LEVEL;
        } else if self.sending || self.peak_left > 0 {
            return; // 전환 중엔 남은 raw 입력이 glow 를 흔들지 않게.
        } else if g.active {
            self.mon = g.monitor;
            self.edge = g.edge;
            self.target_progress = g.progress.clamp(0.0, 1.0) * DWELL_CEILING;
            self.has_target = true;
        } else {
            self.has_target = false;
            self.target_progress = 0.0;
        }
        self.timer_running = true;
    }

    /// setHost 가 실제로 나간 순간. Some = 즉시 최고점 그리기.
    pub fn delivered(&mut self) -> Option<GlowAction> {
        if !self.sending {
            return None;
        }
        self.sending = false;
        self.peak_left = PEAK_TICKS;
        self.timer_running = true;
        Some(GlowAction::Render { progress: 1.0, peak: true })
    }

    /// 전환 실패·건너뜀. 최고점 없이 흐려진다.
    pub fn failed(&mut self) {
        self.sending = false;
        self.peak_left = 0;
        self.has_target = false;
        self.target_progress = 0.0;
        self.timer_running = true;
    }

    /// 16ms 틱.
    pub fn tick(&mut self, now_ms: u64) -> GlowAction {
        if self.peak_left > 0 {
            self.peak_left -= 1;
            if self.peak_left == 0 {
                self.shown = 0.0;
                self.has_target = false;
                self.target_progress = 0.0;
                self.timer_running = false;
                return GlowAction::HideAndStop;
            }
            return GlowAction::None;
        }
        if self.sending && now_ms.saturating_sub(self.send_start_ms) > SEND_TIMEOUT_MS {
            self.sending = false;
            self.has_target = false;
            self.target_progress = 0.0;
        }
        let target = if self.has_target { self.target_progress } else { 0.0 };
        // 오를 때는 빠르게 따라가고, 내릴 때는 부드럽게.
        self.shown += (target - self.shown) * if target > self.shown { 0.5 } else { 0.3 };
        let vis = self.shown.clamp(0.0, 1.0);
        if vis < 0.015 && !self.has_target {
            self.timer_running = false;
            return GlowAction::HideAndStop;
        }
        GlowAction::Render { progress: vis, peak: false }
    }
}

/// 그려진 glow 비트맵(프리멀티플라이드 BGRA, 위→아래 행) + 화면 위치.
pub struct GlowBitmap {
    pub width: i32,
    pub height: i32,
    /// 0xAARRGGBB (프리멀티플라이드).
    pub pixels: Vec<u32>,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy)]
struct Rgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64, // 0..1
}

fn over(dst: Rgba, src: Rgba) -> Rgba {
    let a = src.a + dst.a * (1.0 - src.a);
    if a <= 0.0 {
        return Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
    }
    let mix = |s: f64, d: f64| (s * src.a + d * dst.a * (1.0 - src.a)) / a;
    Rgba {
        r: mix(src.r, dst.r),
        g: mix(src.g, dst.g),
        b: mix(src.b, dst.b),
        a,
    }
}

/// 가로 한 줄(변에 수직인 방향, 폭 MAX_BAND) 색. 길이 방향으로는 같다.
/// GDI+ 그림과 같은 순서: 그라데이션 → tension 코어 → 변 선. x=MAX_BAND-1 이 변.
pub fn canonical_row(progress: f64, peak: bool, accent: Rgb) -> Vec<u32> {
    let w = MAX_BAND;
    let inten = progress.clamp(0.0, 1.0);
    let glow_a = if peak { 235 } else { (inten * 190.0) as i32 };
    let core_t = if peak { 40.0 } else { 2.0 + inten * 32.0 };
    let core_a = if peak { 255 } else { (inten * 240.0) as i32 };
    let core_c = blend(Rgb(255, 255, 255), accent, if peak { 0.75 } else { inten * inten * 0.4 });
    let line_a = if peak { 255 } else { (inten * 255.0) as i32 };
    let line_c = if peak {
        Rgb(255, 255, 255)
    } else {
        blend(Rgb(255, 255, 255), accent, inten * 0.5)
    };

    let mut row = Vec::with_capacity(w as usize);
    for x in 0..w {
        let mut px = Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        if glow_a > 0 {
            // 선형 그라데이션(왼쪽 투명 → 오른쪽 glowA), 픽셀 중심에서 보간.
            let t = (x as f64 + 0.5) / w as f64;
            let a = (glow_a as f64 * t) / 255.0;
            px = over(
                px,
                Rgba {
                    r: accent.0 as f64,
                    g: accent.1 as f64,
                    b: accent.2 as f64,
                    a,
                },
            );
        }
        let cover = |x0: f64| -> f64 {
            let l = (x as f64).max(x0);
            let r = (x as f64 + 1.0).min(w as f64);
            (r - l).clamp(0.0, 1.0)
        };
        let cc = cover(w as f64 - core_t);
        if cc > 0.0 && core_a > 0 {
            px = over(
                px,
                Rgba {
                    r: core_c.0 as f64,
                    g: core_c.1 as f64,
                    b: core_c.2 as f64,
                    a: core_a as f64 / 255.0 * cc,
                },
            );
        }
        let lc = cover(w as f64 - 2.5);
        if lc > 0.0 && line_a > 0 {
            px = over(
                px,
                Rgba {
                    r: line_c.0 as f64,
                    g: line_c.1 as f64,
                    b: line_c.2 as f64,
                    a: line_a as f64 / 255.0 * lc,
                },
            );
        }
        let a8 = (px.a * 255.0).round().clamp(0.0, 255.0) as u32;
        let pm = |c: f64| ((c * px.a).round().clamp(0.0, 255.0)) as u32;
        row.push((a8 << 24) | (pm(px.r) << 16) | (pm(px.g) << 8) | pm(px.b));
    }
    row
}

/// 변 방향대로 회전한 비트맵과 위치. C#: Left=좌우 반전 · Bottom=90° · Top=270° · Right=그대로.
pub fn render(mon: &HandoffRect, edge: HandoffEdge, progress: f64, peak: bool, accent: Rgb) -> Option<GlowBitmap> {
    let horizontal = matches!(edge, HandoffEdge::Top | HandoffEdge::Bottom);
    let length = if horizontal { mon.width } else { mon.height };
    if length <= 0 {
        return None;
    }
    let row = canonical_row(progress, peak, accent);
    let w = MAX_BAND as usize;
    let len = length as usize;
    match edge {
        HandoffEdge::Right | HandoffEdge::Left => {
            let line: Vec<u32> = if edge == HandoffEdge::Right { row } else { row.into_iter().rev().collect() };
            let mut pixels = Vec::with_capacity(w * len);
            for _ in 0..len {
                pixels.extend_from_slice(&line);
            }
            let x = if edge == HandoffEdge::Right { mon.right() - MAX_BAND } else { mon.left };
            Some(GlowBitmap {
                width: MAX_BAND,
                height: length,
                pixels,
                x,
                y: mon.top,
            })
        }
        HandoffEdge::Bottom | HandoffEdge::Top => {
            // Bottom: 시계 방향 90° → 변(오른쪽)이 아래로. Top: 270° → 변이 위로.
            let mut pixels = Vec::with_capacity(w * len);
            for yy in 0..w {
                let v = if edge == HandoffEdge::Bottom { row[yy] } else { row[w - 1 - yy] };
                for _ in 0..len {
                    pixels.push(v);
                }
            }
            let y = if edge == HandoffEdge::Bottom { mon.bottom() - MAX_BAND } else { mon.top };
            Some(GlowBitmap {
                width: length,
                height: MAX_BAND,
                pixels,
                x: mon.left,
                y,
            })
        }
    }
}
