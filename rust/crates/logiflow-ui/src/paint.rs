//! 그리기 도우미 — C# 커스텀 페인팅 컨트롤(ThemedButton·Toggle·Segmented·NavButton·Card·Badge·IconView)과
//! Icons(24-unit 라인 아이콘)를 같은 좌표·같은 도형으로. 좌표 단위 = WinForms 픽셀(96dpi) = egui point.

use egui::epaint::{CubicBezierShape, PathShape, PathStroke};
use egui::{Align2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, Id, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use logiflow_core::theme::{blend, Palette, Rgb};

pub fn c(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.0, rgb.1, rgb.2)
}

pub const MIN_PT: f32 = 9.5;

/// C# UiFont.Get(basePt) — pt = base + 단계 가산, 최소 9.5pt. egui 크기 = em(px) = pt × 96/72.
pub fn font(base_pt: f32, offset: i32, bold: bool) -> FontId {
    let pt = (base_pt + offset as f32).max(MIN_PT);
    let px = pt * 96.0 / 72.0;
    if bold {
        FontId::new(px, FontFamily::Name("bold".into()))
    } else {
        FontId::new(px, FontFamily::Proportional)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Default,
    Primary,
    Ghost,
    Icon,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Mark,
    Swap,
    Gear,
    Diag,
    Refresh,
    Folder,
    Plug,
    Mouse,
    Keyboard,
    Pencil,
    Edge,
    Monitor,
    Dot,
}

/// 한 화면(페이지)을 그리는 맥락. origin = 페이지 (0,0) 의 화면 위치.
pub struct Pc<'a> {
    pub ui: &'a Ui,
    pub pal: &'static Palette,
    pub off: i32,
    pub origin: Pos2,
    pub interactive: bool,
    pub id_base: Id,
}

impl<'a> Pc<'a> {
    pub fn painter(&self) -> &Painter {
        self.ui.painter()
    }
    pub fn r(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.origin + Vec2::new(x, y), Vec2::new(w, h))
    }
    pub fn f(&self, base: f32, bold: bool) -> FontId {
        font(base, self.off, bold)
    }
    pub fn id(&self, key: &str) -> Id {
        self.id_base.with(key)
    }

    pub fn measure(&self, text: &str, base: f32, bold: bool) -> f32 {
        self.painter().layout_no_wrap(text.to_string(), self.f(base, bold), Color32::WHITE).size().x
    }
    /// TextRenderer.MeasureText 근사(좌우 GDI 여백 포함).
    pub fn measure_gdi(&self, text: &str, base: f32, bold: bool) -> f32 {
        let f = self.f(base, bold);
        self.measure(text, base, bold) + f.size / 3.0
    }

    /// 왼쪽 위 기준 한 줄 글자. 폭을 돌려준다.
    pub fn text(&self, x: f32, y: f32, text: &str, base: f32, bold: bool, color: Rgb) -> f32 {
        let p = self.origin + Vec2::new(x, y);
        self.painter().text(p, Align2::LEFT_TOP, text, self.f(base, bold), c(color)).width()
    }

    /// 사각형 안에 정렬해 그리기(C# StringFormat Center/Near).
    pub fn text_in(&self, rect: Rect, text: &str, font: FontId, color: Color32, align: Align2) {
        let pos = align.pos_in_rect(&rect);
        self.painter().text(pos, align, text, font, color);
    }

    /// 고정 폭·높이 라벨(AutoSize=false). 줄바꿈하고 높이 밖은 잘림.
    pub fn text_box(&self, x: f32, y: f32, w: f32, h: f32, text: &str, base: f32, bold: bool, color: Rgb) {
        let rect = self.r(x, y, w, h);
        let galley = self.painter().layout(text.replace("\r\n", "\n"), self.f(base, bold), c(color), w);
        self.painter()
            .with_clip_rect(rect.intersect(self.painter().clip_rect()))
            .galley(rect.min, galley, c(color));
    }

    /// 한 줄, 넘치면 … (AutoEllipsis).
    pub fn text_ellipsis(&self, x: f32, y: f32, w: f32, text: &str, base: f32, bold: bool, color: Rgb) {
        let f = self.f(base, bold);
        let mut s = text.to_string();
        if self.painter().layout_no_wrap(s.clone(), f.clone(), Color32::WHITE).size().x > w {
            let chars: Vec<char> = text.chars().collect();
            let mut n = chars.len();
            while n > 0 {
                n -= 1;
                s = chars[..n].iter().collect::<String>() + "…";
                if self.painter().layout_no_wrap(s.clone(), f.clone(), Color32::WHITE).size().x <= w {
                    break;
                }
            }
        }
        let rect = self.r(x, y, w, 40.0);
        self.painter()
            .with_clip_rect(rect.intersect(self.painter().clip_rect()))
            .text(rect.min, Align2::LEFT_TOP, s, f, c(color));
    }

    /// 1px 구분선(WinForms Panel 높이 1).
    pub fn hline(&self, x: f32, y: f32, w: f32) {
        self.painter().rect_filled(self.r(x, y, w, 1.0), 0, c(self.pal.divider));
    }

    pub fn fill(&self, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
        self.painter().rect_filled(self.r(x, y, w, h), 0, c(color));
    }

    /// C# 컨트롤 그림 사각형 = (0.5, 0.5, W-1.5, H-1.5).
    fn inset(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        self.r(x + 0.5, y + 0.5, w - 1.5, h - 1.5)
    }

    fn hover_hand(&self, resp: &egui::Response) {
        if resp.hovered() && self.interactive {
            self.ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }

    pub fn click_area(&self, key: &str, rect: Rect) -> egui::Response {
        self.ui
            .interact(rect, self.id(key), if self.interactive { Sense::click() } else { Sense::hover() })
    }

    /// ThemedButton. enabled=false 면 같은 모양으로 그리되 눌리지 않는다(C# 과 같게).
    #[allow(clippy::too_many_arguments)]
    pub fn button(&self, key: &str, x: f32, y: f32, w: f32, h: f32, text: &str, icon: Option<Icon>, variant: Variant, enabled: bool) -> bool {
        let rect = self.r(x, y, w, h);
        let resp = self.click_area(key, rect);
        if enabled {
            self.hover_hand(&resp);
        }
        let hover = resp.hovered() && self.interactive && enabled;
        let p = self.pal;
        let (bg, border, fg) = match variant {
            Variant::Primary => (Some(p.accent), Some(p.accent), Rgb(255, 255, 255)),
            Variant::Ghost => (None, None, p.accent),
            Variant::Icon => (Some(if hover { p.hover } else { p.section }), Some(p.border), p.muted),
            Variant::Default => (Some(if hover { p.hover } else { p.section }), Some(p.border), p.ink),
        };
        let ir = self.inset(x, y, w, h);
        if let Some(bg) = bg {
            self.painter().rect_filled(ir, CornerRadius::same(7), c(bg));
        }
        if let Some(b) = border {
            self.painter()
                .rect_stroke(ir, CornerRadius::same(7), Stroke::new(1.0, c(b)), StrokeKind::Middle);
        }
        let f = self.f(9.0, false);
        let icon_size = 14.0;
        match (icon, text.is_empty()) {
            (Some(ic), true) => {
                let r = Rect::from_center_size(rect.center(), Vec2::splat(icon_size));
                draw_icon(self.painter(), ic, r, c(fg));
            }
            (Some(ic), false) => {
                let tw = self.measure(text, 9.0, false);
                let total = icon_size + 6.0 + tw;
                let lx = rect.left() + (w - total) / 2.0;
                let r = Rect::from_min_size(Pos2::new(lx, rect.top() + (h - icon_size) / 2.0), Vec2::splat(icon_size));
                draw_icon(self.painter(), ic, r, c(fg));
                self.painter()
                    .text(Pos2::new(lx + icon_size + 6.0, rect.center().y), Align2::LEFT_CENTER, text, f, c(fg));
            }
            (None, false) => self.text_in(rect, text, f, c(fg), Align2::CENTER_CENTER),
            (None, true) => {}
        }
        enabled && self.interactive && resp.clicked()
    }

    /// 40×22 토글. 눌리면 true(값 바꾸기는 호출 측).
    pub fn toggle(&self, key: &str, x: f32, y: f32, checked: bool) -> bool {
        let (w, h) = (40.0, 22.0);
        let rect = self.r(x, y, w, h);
        let resp = self.click_area(key, rect);
        self.hover_hand(&resp);
        let ir = self.inset(x, y, w, h);
        let rad = (ir.height() / 2.0).round() as u8;
        self.painter()
            .rect_filled(ir, CornerRadius::same(rad), c(if checked { self.pal.accent } else { self.pal.toggle_off }));
        let d = h - 4.0;
        let kx = if checked { w - d - 2.0 } else { 2.0 };
        let center = rect.min + Vec2::new(kx + d / 2.0, 2.0 + d / 2.0);
        self.painter().circle_filled(center, d / 2.0, Color32::WHITE);
        self.interactive && resp.clicked()
    }

    /// 세그먼트 선택. 누른 칸 번호.
    #[allow(clippy::too_many_arguments)]
    pub fn segmented(&self, key: &str, x: f32, y: f32, w: f32, h: f32, items: &[&str], selected: usize) -> Option<usize> {
        let rect = self.r(x, y, w, h);
        let resp = self.click_area(key, rect);
        self.hover_hand(&resp);
        let ir = self.inset(x, y, w, h);
        self.painter().rect_filled(ir, CornerRadius::same(8), c(self.pal.chip));
        let seg_w = w / items.len().max(1) as f32;
        // 세그먼트 글꼴 = 부모(폼) 글꼴 10pt.
        let f = self.f(10.0, false);
        for (i, it) in items.iter().enumerate() {
            if i == selected {
                let sr = self.r(x + i as f32 * seg_w + 3.0, y + 3.0, seg_w - 6.0, h - 6.0);
                self.painter().rect_filled(sr, CornerRadius::same(6), c(self.pal.section));
            }
            let tr = self.r(x + i as f32 * seg_w, y, seg_w, h);
            self.text_in(
                tr,
                it,
                f.clone(),
                c(if i == selected { self.pal.ink } else { self.pal.muted }),
                Align2::CENTER_CENTER,
            );
        }
        if self.interactive && resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let idx = (((p.x - rect.left()) / seg_w.max(1.0)) as i32).clamp(0, items.len() as i32 - 1) as usize;
                if idx != selected {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// ThemedCard 바탕(Section + 1px Border, 반지름 10).
    pub fn card(&self, x: f32, y: f32, w: f32, h: f32) {
        let full = self.r(x, y, w, h);
        self.painter().rect_filled(full, CornerRadius::same(10), c(self.pal.section));
        self.painter().rect_stroke(
            self.inset(x, y, w, h),
            CornerRadius::same(10),
            Stroke::new(1.0, c(self.pal.border)),
            StrokeKind::Middle,
        );
    }

    /// 알약 배지. 폭을 돌려준다(AutoWidth = 글자 + 20).
    pub fn badge(&self, x: f32, y: f32, h: f32, text: &str, accent: bool) -> f32 {
        let w = self.measure_gdi(text, 8.5, true) + 20.0;
        let ir = self.inset(x, y, w, h);
        let (bg, fg) = if accent {
            (self.pal.accent_soft(), self.pal.accent)
        } else {
            (self.pal.chip, self.pal.muted)
        };
        self.painter().rect_filled(ir, CornerRadius::same((h / 2.0) as u8), c(bg));
        self.text_in(self.r(x, y, w, h), text, self.f(8.5, true), c(fg), Align2::CENTER_CENTER);
        w
    }

    pub fn icon(&self, ic: Icon, x: f32, y: f32, size: f32, color: Rgb) {
        draw_icon(self.painter(), ic, self.r(x, y, size, size), c(color));
    }
}

/// 사이드바 내비 항목. 눌리면 true.
pub fn nav_button(pc: &Pc<'_>, key: &str, x: f32, y: f32, w: f32, h: f32, text: &str, icon: Icon, active: bool) -> bool {
    let rect = pc.r(x, y, w, h);
    let resp = pc.click_area(key, rect);
    pc.hover_hand(&resp);
    let ir = pc.inset(x, y, w, h);
    if active {
        pc.painter().rect_filled(ir, CornerRadius::same(8), c(pc.pal.accent_soft()));
    } else if resp.hovered() && pc.interactive {
        pc.painter().rect_filled(ir, CornerRadius::same(8), c(pc.pal.hover));
    }
    let fg = if active { pc.pal.accent } else { pc.pal.ink };
    let ic = if active { pc.pal.accent } else { pc.pal.muted };
    draw_icon(pc.painter(), icon, pc.r(x + 10.0, y + (h - 18.0) / 2.0, 18.0, 18.0), c(ic));
    pc.text_in(pc.r(x + 36.0, y, w - 38.0, h), text, pc.f(9.5, false), c(fg), Align2::LEFT_CENTER);
    pc.interactive && resp.clicked()
}

// ---------------- 아이콘 ----------------

/// 둥근 사각형 꼭짓점들(반지름 실수, 모서리당 8조각). 아이콘·점선 박스 공용.
pub fn rounded_points(r: Rect, rad: f32) -> Vec<Pos2> {
    let rad = rad.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    let mut pts = Vec::new();
    let corners = [
        (Pos2::new(r.left() + rad, r.top() + rad), 180.0f32),
        (Pos2::new(r.right() - rad, r.top() + rad), 270.0),
        (Pos2::new(r.right() - rad, r.bottom() - rad), 0.0),
        (Pos2::new(r.left() + rad, r.bottom() - rad), 90.0),
    ];
    for (ctr, start) in corners {
        for k in 0..=8 {
            let a = (start + 90.0 * k as f32 / 8.0).to_radians();
            pts.push(Pos2::new(ctr.x + rad * a.cos(), ctr.y + rad * a.sin()));
        }
    }
    pts
}

fn arc_points(cx: f32, cy: f32, rx: f32, ry: f32, start_deg: f32, sweep_deg: f32) -> Vec<Pos2> {
    let n = ((sweep_deg.abs() / 10.0).ceil() as usize).max(4);
    (0..=n)
        .map(|k| {
            let a = (start_deg + sweep_deg * k as f32 / n as f32).to_radians();
            Pos2::new(cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

pub fn draw_icon(p: &Painter, ic: Icon, r: Rect, col: Color32) {
    let u = r.width().min(r.height()) / 24.0;
    let pt = |x: f32, y: f32| Pos2::new(r.left() + x / 24.0 * r.width(), r.top() + y / 24.0 * r.height());
    let pen = |w: f32| Stroke::new((w * u).max(1.0), col);
    let line = |pts: Vec<Pos2>, w: f32| {
        p.add(Shape::Path(PathShape::line(pts, PathStroke::new((w * u).max(1.0), col))));
    };
    let rounded = |x: f32, y: f32, w: f32, h: f32, rad: f32, sw: f32| {
        let rect = Rect::from_min_size(Pos2::new(r.left() + x * u, r.top() + y * u), Vec2::new(w * u, h * u));
        p.add(Shape::Path(PathShape::closed_line(
            rounded_points(rect, rad * u),
            PathStroke::new((sw * u).max(1.0), col),
        )));
    };
    match ic {
        Icon::Mark => {
            rounded(2.3, 9.2, 8.3, 7.8, 2.0, 1.9);
            rounded(13.4, 9.2, 8.3, 7.8, 2.0, 1.9);
            p.add(CubicBezierShape::from_points_stroke(
                [pt(8.6, 9.2), pt(9.4, 4.9), pt(14.6, 4.9), pt(15.4, 9.2)],
                false,
                Color32::TRANSPARENT,
                pen(1.9),
            ));
            p.add(Shape::convex_polygon(vec![pt(15.7, 9.6), pt(13.1, 8.7), pt(14.4, 11.1)], col, Stroke::NONE));
        }
        Icon::Swap => {
            line(vec![pt(4.0, 8.0), pt(17.0, 8.0)], 1.7);
            line(vec![pt(13.0, 4.0), pt(17.0, 8.0), pt(13.0, 12.0)], 1.7);
            line(vec![pt(20.0, 16.0), pt(7.0, 16.0)], 1.7);
            line(vec![pt(11.0, 20.0), pt(7.0, 16.0), pt(11.0, 12.0)], 1.7);
        }
        Icon::Gear => {
            let ctr = pt(12.0, 12.0);
            p.circle_stroke(ctr, 3.2 * u, pen(1.7));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::PI / 4.0;
                let a1 = Pos2::new(ctr.x + a.cos() * 5.0 * u, ctr.y + a.sin() * 5.0 * u);
                let a2 = Pos2::new(ctr.x + a.cos() * 8.0 * u, ctr.y + a.sin() * 8.0 * u);
                p.line_segment([a1, a2], pen(1.7));
            }
        }
        Icon::Diag => line(
            vec![pt(3.0, 12.0), pt(6.0, 12.0), pt(8.5, 18.0), pt(12.5, 5.0), pt(15.0, 12.0), pt(21.0, 12.0)],
            1.7,
        ),
        Icon::Refresh => {
            let ctr = pt(12.0, 12.0);
            line(arc_points(ctr.x, ctr.y, 8.0 * u, 8.0 * u, -60.0, 300.0), 1.7);
            line(vec![pt(20.0, 4.0), pt(20.0, 9.0), pt(15.0, 9.0)], 1.7);
        }
        Icon::Folder => {
            p.add(Shape::Path(PathShape::closed_line(
                vec![pt(3.0, 7.0), pt(3.0, 17.0), pt(21.0, 17.0), pt(21.0, 9.0), pt(11.0, 9.0), pt(9.0, 7.0)],
                PathStroke::new((1.7 * u).max(1.0), col),
            )));
        }
        Icon::Plug => {
            p.line_segment([pt(9.0, 3.0), pt(9.0, 8.0)], pen(1.7));
            p.line_segment([pt(15.0, 3.0), pt(15.0, 8.0)], pen(1.7));
            let left = pt(6.5, 8.0).x;
            let top = pt(8.0, 8.0).y;
            let d = 11.0 * u;
            let mut path = vec![pt(6.5, 8.0), pt(17.5, 8.0), pt(17.5, 11.0)];
            path.extend(arc_points(left + d / 2.0, top + d / 2.0, d / 2.0, d / 2.0, 0.0, 180.0));
            line(path, 1.7);
            p.line_segment([pt(12.0, 16.5), pt(12.0, 21.0)], pen(1.7));
        }
        Icon::Mouse => {
            rounded(7.0, 3.0, 10.0, 18.0, 5.0, 1.7);
            p.line_segment([pt(12.0, 7.0), pt(12.0, 10.0)], pen(1.7));
        }
        Icon::Keyboard => {
            rounded(2.5, 6.5, 19.0, 11.0, 2.0, 1.7);
            for x in [6.0, 9.0, 12.0, 15.0, 18.0] {
                let a = pt(x, 10.0);
                p.line_segment([a, Pos2::new(a.x + 0.4, a.y)], pen(1.7));
            }
            p.line_segment([pt(7.0, 14.0), pt(17.0, 14.0)], pen(1.7));
        }
        Icon::Pencil => {
            p.add(Shape::Path(PathShape::closed_line(
                vec![pt(4.0, 20.0), pt(8.0, 20.0), pt(19.0, 9.0), pt(15.0, 5.0), pt(4.0, 16.0)],
                PathStroke::new((1.7 * u).max(1.0), col),
            )));
            p.line_segment([pt(14.0, 6.0), pt(18.0, 10.0)], pen(1.7));
        }
        Icon::Edge => {
            rounded(3.0, 4.0, 13.0, 10.0, 1.6, 1.7);
            p.line_segment([pt(16.0, 9.0), pt(21.0, 9.0)], pen(1.7));
            line(vec![pt(18.0, 6.0), pt(21.0, 9.0), pt(18.0, 12.0)], 1.7);
        }
        Icon::Monitor => {
            rounded(3.0, 4.0, 18.0, 12.0, 2.0, 1.7);
            p.line_segment([pt(8.0, 20.0), pt(16.0, 20.0)], pen(1.7));
            p.line_segment([pt(12.0, 16.0), pt(12.0, 20.0)], pen(1.7));
        }
        Icon::Dot => {
            // 상태 점(7×7, 세로 가운데).
            p.circle_filled(Pos2::new(r.left() + 3.5, r.top() + r.height() / 2.0), 3.5, col);
        }
    }
}

/// C# Theme.Blend(Muted, Content, a) — 스크롤 손잡이 색.
pub fn thumb_color(pal: &Palette, hot: bool) -> Color32 {
    c(blend(pal.muted, pal.content, if hot { 0.55 } else { 0.35 }))
}
