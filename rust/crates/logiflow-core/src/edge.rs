//! 가장자리 판정과 캔버스 배치 계산 — C# `HandoffPrimitives`·`EdgeDetector`·`HandoffLayout` 이식
//! (원 출처 LocalDex InputHandoff). 좌표는 같은 좌표계(논리 픽셀)여야 한다.

/// PC 데스크톱에서 다른 채널 PC 가 놓인 변.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HandoffEdge {
    Left = 0,
    Right = 1,
    Top = 2,
    Bottom = 3,
}

impl HandoffEdge {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(HandoffEdge::Left),
            1 => Some(HandoffEdge::Right),
            2 => Some(HandoffEdge::Top),
            3 => Some(HandoffEdge::Bottom),
            _ => None,
        }
    }
    /// C# enum ToString() 과 같은 이름(로그 문구용).
    pub fn name(self) -> &'static str {
        match self {
            HandoffEdge::Left => "Left",
            HandoffEdge::Right => "Right",
            HandoffEdge::Top => "Top",
            HandoffEdge::Bottom => "Bottom",
        }
    }
}

/// 논리 좌표 사각형(모니터·박스). 폭·높이는 최소 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HandoffRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl HandoffRect {
    pub fn new(left: i32, top: i32, width: i32, height: i32) -> Self {
        HandoffRect {
            left,
            top,
            width: width.max(1),
            height: height.max(1),
        }
    }
    pub fn right(&self) -> i32 {
        self.left + self.width
    }
    pub fn bottom(&self) -> i32 {
        self.top + self.height
    }
}

pub const DEFAULT_DWELL_MS: i32 = 330;
pub const DEFAULT_BAND_PX: i32 = 2;
pub const DEFAULT_MIN_PUSH_PX: i32 = 10;
pub const REARM_DISTANCE: i32 = 16;

/// 순수 엣지-푸시 판정기. 좁은 띠(band) 안에서 dwell 유지 + 바깥 방향 누적 푸시가 차면 1회 true.
/// 버튼 누름(드래그)·주입 입력은 트리거하지 않는다. 전환 뒤 변에서 떨어져야 재무장.
#[derive(Clone, Debug)]
pub struct EdgeDetector {
    pub dwell_ms: i32,
    pub band_px: i32,
    pub min_push_px: i32,
    touch_start_ms: i64,
    push_accum: i32,
    armed: bool,
}

impl Default for EdgeDetector {
    fn default() -> Self {
        EdgeDetector {
            dwell_ms: DEFAULT_DWELL_MS,
            band_px: DEFAULT_BAND_PX,
            min_push_px: DEFAULT_MIN_PUSH_PX,
            touch_start_ms: -1,
            push_accum: 0,
            armed: true,
        }
    }
}

impl EdgeDetector {
    pub fn new(dwell_ms: i32, min_push_px: i32) -> Self {
        EdgeDetector {
            dwell_ms,
            min_push_px,
            ..Default::default()
        }
    }

    /// 전환 직후 변에서 REARM_DISTANCE 만큼 떨어지기 전까지 false.
    pub fn armed(&self) -> bool {
        self.armed
    }

    pub fn progress(&self, time_ms: i64) -> f64 {
        if self.touch_start_ms < 0 || self.dwell_ms <= 0 {
            return 0.0;
        }
        let elapsed = time_ms - self.touch_start_ms;
        if elapsed <= 0 {
            return 0.0;
        }
        if elapsed >= self.dwell_ms as i64 {
            1.0
        } else {
            elapsed as f64 / self.dwell_ms as f64
        }
    }

    pub fn reset(&mut self, require_leave_edge: bool) {
        self.touch_start_ms = -1;
        self.push_accum = 0;
        self.armed = !require_leave_edge;
    }

    pub fn distance_from_edge(bounds: &HandoffRect, edge: HandoffEdge, x: i32, y: i32) -> i32 {
        match edge {
            HandoffEdge::Left => x - bounds.left,
            HandoffEdge::Right => bounds.right() - 1 - x,
            HandoffEdge::Top => y - bounds.top,
            HandoffEdge::Bottom => bounds.bottom() - 1 - y,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn feed(
        &mut self,
        bounds: &HandoffRect,
        edge: HandoffEdge,
        x: i32,
        y: i32,
        outward_push: i32,
        time_ms: i64,
        injected: bool,
        button_down: bool,
    ) -> bool {
        if injected {
            return false;
        }
        let distance = Self::distance_from_edge(bounds, edge, x, y);
        if !self.armed {
            if distance >= REARM_DISTANCE {
                self.armed = true;
            }
            return false;
        }
        let at_band = distance <= self.band_px;
        if button_down || !at_band {
            self.touch_start_ms = -1;
            self.push_accum = 0;
            return false;
        }
        if self.touch_start_ms < 0 {
            self.touch_start_ms = time_ms;
            self.push_accum = 0;
            return false;
        }
        if outward_push > 0 {
            self.push_accum += outward_push;
        }
        if time_ms - self.touch_start_ms < self.dwell_ms as i64 {
            return false;
        }
        if self.push_accum < self.min_push_px {
            return false;
        }
        self.reset(true);
        true
    }

    /// raw 이동(dx,dy) 중 해당 변 바깥으로 향하는 성분.
    pub fn outward_push(edge: HandoffEdge, dx: i32, dy: i32) -> i32 {
        match edge {
            HandoffEdge::Left => {
                if dx < 0 {
                    -dx
                } else {
                    0
                }
            }
            HandoffEdge::Right => dx.max(0),
            HandoffEdge::Top => {
                if dy < 0 {
                    -dy
                } else {
                    0
                }
            }
            HandoffEdge::Bottom => dy.max(0),
        }
    }
}

pub const DEFAULT_SNAP_PX: i32 = 24;

fn overlap(a1: i32, a2: i32, b1: i32, b2: i32) -> i32 {
    0.max(a2.min(b2) - a1.max(b1))
}

/// 박스에 가장 가까운 모니터 변. snap_px 안이면 Some((모니터 번호, 변)).
pub fn resolve_attachment(monitors: &[HandoffRect], b: &HandoffRect, snap_px: i32) -> Option<(usize, HandoffEdge)> {
    let mut best = f64::MAX;
    let mut found: Option<(usize, HandoffEdge)> = None;
    let mut consider = |ov: i32, distance: f64, index: usize, cand: HandoffEdge, best: &mut f64| {
        if ov <= 0 || distance >= *best {
            return;
        }
        *best = distance;
        found = Some((index, cand));
    };
    for (i, m) in monitors.iter().enumerate() {
        let v = overlap(b.top, b.bottom(), m.top, m.bottom());
        let h = overlap(b.left, b.right(), m.left, m.right());
        consider(v, (b.left - m.right()).abs() as f64, i, HandoffEdge::Right, &mut best);
        consider(v, (m.left - b.right()).abs() as f64, i, HandoffEdge::Left, &mut best);
        consider(h, (b.top - m.bottom()).abs() as f64, i, HandoffEdge::Bottom, &mut best);
        consider(h, (m.top - b.bottom()).abs() as f64, i, HandoffEdge::Top, &mut best);
    }
    match found {
        Some(f) if best <= snap_px as f64 => Some(f),
        _ => None,
    }
}

/// 박스를 모니터 변에 밀착(변 따라 가운데 정렬).
pub fn snap_flush(monitor: &HandoffRect, edge: HandoffEdge, box_w: i32, box_h: i32) -> HandoffRect {
    let (left, top) = match edge {
        HandoffEdge::Right => (monitor.right(), monitor.top + (monitor.height - box_h) / 2),
        HandoffEdge::Left => (monitor.left - box_w, monitor.top + (monitor.height - box_h) / 2),
        HandoffEdge::Top => (monitor.left + (monitor.width - box_w) / 2, monitor.top - box_h),
        HandoffEdge::Bottom => (monitor.left + (monitor.width - box_w) / 2, monitor.bottom()),
    };
    HandoffRect::new(left, top, box_w, box_h)
}
