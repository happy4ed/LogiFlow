//! 전환 관리 캔버스 — C# `MonitorLayoutPanel` 이식. 이 PC 모니터를 실제 배치로 그리고, 다른 채널 PC 박스
//! (모니터 크기 사각형)를 가장자리에 끌어 붙인다. 붙음 = 자동(파랑 실선) · 떨어짐 = 수동(회색 점선).
//! rs.3(첫 사용 점검 반영): 아래 줄 상자의 상태 글자가 잘리지 않게 자리를 비우고, 이름이 기본값이면 「채널 N」만,
//! 모니터에는 이 PC 의 채널을 적고, 상태 글자는 파랑(누를 수 있는 것) 대신 보조색, 상자 위에서는 손 모양 커서.

use crate::paint::{c, draw_icon, font, rounded_points, Icon};
use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use logiflow_core::config::EdgeAttachment;
use logiflow_core::edge::{resolve_attachment, snap_flush, HandoffEdge, HandoffRect};
use logiflow_core::theme::Palette;

const PAD: f32 = 14.0;
const SNAP_PX: i32 = 30;
const MIN_SCALE: f64 = 0.035;
const MAX_SCALE: f64 = 0.16;

#[derive(Clone, Debug)]
pub struct Screen {
    pub name: String,
    pub bounds: HandoffRect,
    pub primary: bool,
}

#[derive(Clone, Debug)]
struct ChannelBox {
    channel: i32,
    placed: bool,
    monitor_index: usize,
    edge: HandoffEdge,
    /// 미배치·끄는 중 위치(캔버스 좌표). None = 기본 자리.
    rect: Option<Rect>,
}

#[derive(Default)]
pub struct Canvas {
    boxes: Vec<ChannelBox>,
    screens: Vec<Screen>,
    primary: usize,
    v_left: i32,
    v_top: i32,
    v_w: i32,
    v_h: i32,
    scale: f64,
    size: Vec2,
    dragging: Option<usize>,
    drag_offset: Vec2,
    key: String,
    mon_rects: Vec<HandoffRect>,
    box_w: f32,
    box_h: f32,
    /// 상자 아래 상태 글자 한 줄 높이(아래 줄 상자가 캔버스 밖으로 글자를 밀어내지 않게).
    sub_h: f32,
}

impl Canvas {
    /// C# SetData — 채널·현재 채널·기존 배치·모니터가 바뀌었을 때만 다시 만든다(끄는 중엔 유지).
    pub fn sync(&mut self, channels: &[i32], current: i32, existing: &[EdgeAttachment], screens: Vec<Screen>) {
        let key = format!(
            "{:?}|{}|{:?}|{:?}",
            channels,
            current,
            existing,
            screens.iter().map(|s| (&s.name, s.bounds)).collect::<Vec<_>>()
        );
        if key == self.key || self.dragging.is_some() {
            return;
        }
        self.key = key;
        self.primary = screens.iter().position(|s| s.primary).unwrap_or(0);
        self.screens = screens;
        self.boxes = channels
            .iter()
            .map(|&ch| {
                let mut b = ChannelBox {
                    channel: ch,
                    placed: false,
                    monitor_index: 0,
                    edge: HandoffEdge::Right,
                    rect: None,
                };
                if let Some(a) = existing.iter().find(|a| a.channel == ch) {
                    if let Some(idx) = self.screens.iter().position(|s| s.name == a.monitor_device_name) {
                        b.placed = true;
                        b.monitor_index = idx;
                        b.edge = a.edge;
                    }
                }
                b
            })
            .collect();
        self.compute_virtual();
    }

    /// 모니터 가상 경계(채널 박스가 붙을 여백 포함 — 가로 1.2칸·세로 1.5칸).
    fn compute_virtual(&mut self) {
        if self.screens.is_empty() {
            self.v_left = 0;
            self.v_top = 0;
            self.v_w = 1;
            self.v_h = 1;
            return;
        }
        let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        let (mut sw, mut sh) = (0f64, 0f64);
        for s in &self.screens {
            let bd = s.bounds;
            l = l.min(bd.left);
            t = t.min(bd.top);
            r = r.max(bd.right());
            b = b.max(bd.bottom());
            sw += bd.width as f64;
            sh += bd.height as f64;
        }
        let n = self.screens.len() as f64;
        let (mx, my) = ((sw / n * 1.2) as i32, (sh / n * 1.5) as i32);
        self.v_left = l - mx;
        self.v_top = t - my;
        self.v_w = (r + mx - self.v_left).max(1);
        self.v_h = (b + my - self.v_top).max(1);
    }

    /// C# RelayoutToHost — 뷰포트에 꼭 맞는 스케일(상·하한).
    pub fn relayout(&mut self, viewport: Vec2) {
        if self.screens.is_empty() {
            self.size = viewport.max(Vec2::splat(1.0));
            return;
        }
        self.compute_virtual();
        let fit_w = (viewport.x as f64 - 2.0 * PAD as f64) / self.v_w as f64;
        let fit_h = (viewport.y as f64 - 2.0 * PAD as f64) / self.v_h as f64;
        self.scale = fit_w.min(fit_h).clamp(MIN_SCALE, MAX_SCALE);
        let need_w = (self.v_w as f64 * self.scale) as f32 + 2.0 * PAD;
        let need_h = (self.v_h as f64 * self.scale) as f32 + 2.0 * PAD;
        self.size = Vec2::new(viewport.x.max(need_w), viewport.y.max(need_h));
    }

    pub fn clear_placements(&mut self) -> Vec<EdgeAttachment> {
        for b in &mut self.boxes {
            b.placed = false;
            b.rect = None;
        }
        self.attachments()
    }

    pub fn attachments(&self) -> Vec<EdgeAttachment> {
        self.boxes
            .iter()
            .filter(|b| b.placed && b.monitor_index < self.screens.len())
            .map(|b| EdgeAttachment {
                channel: b.channel,
                monitor_device_name: self.screens[b.monitor_index].name.clone(),
                edge: b.edge,
            })
            .collect()
    }

    fn recompute_monitor_rects(&mut self) {
        self.mon_rects.clear();
        if self.screens.is_empty() {
            return;
        }
        let drawn_w = self.v_w as f64 * self.scale;
        let drawn_h = self.v_h as f64 * self.scale;
        let off_x = (PAD as f64).max((self.size.x as f64 - drawn_w) / 2.0);
        let off_y = (PAD as f64).max((self.size.y as f64 - drawn_h) / 2.0);
        let (mut sw, mut sh) = (0f64, 0f64);
        for s in &self.screens {
            let b = s.bounds;
            let px = (off_x + (b.left - self.v_left) as f64 * self.scale) as i32;
            let py = (off_y + (b.top - self.v_top) as f64 * self.scale) as i32;
            let pw = (b.width as f64 * self.scale) as i32;
            let ph = (b.height as f64 * self.scale) as i32;
            self.mon_rects.push(HandoffRect::new(px, py, pw, ph));
            sw += pw as f64;
            sh += ph as f64;
        }
        // 채널 박스 = 모니터 평균 크기.
        self.box_w = (sw / self.screens.len() as f64) as f32;
        self.box_h = (sh / self.screens.len() as f64) as f32;
    }

    fn display_rect(&self, i: usize, tray_index: usize) -> Rect {
        let b = &self.boxes[i];
        if Some(i) == self.dragging || !b.placed {
            if let Some(r) = b.rect {
                return r;
            }
            // 미배치 박스는 아래 한 줄에 모두 들어오도록 폭을 줄인다.
            let count = self
                .boxes
                .iter()
                .enumerate()
                .filter(|(j, x)| !x.placed && Some(*j) != self.dragging)
                .count()
                .max(1) as f32;
            let gap = 12.0;
            let avail = self.size.x - 2.0 * PAD - (count - 1.0) * gap;
            let tw = (self.box_w * 0.78).min(avail / count).max(1.0);
            let aspect = if self.box_w > 1.0 { self.box_h / self.box_w } else { 0.6 };
            let th = tw * aspect;
            let x = PAD + tray_index as f32 * (tw + gap);
            let y = self.size.y - th - PAD - self.sub_h - 4.0;
            return Rect::from_min_size(Pos2::new(x, y), Vec2::new(tw, th));
        }
        if b.monitor_index >= self.mon_rects.len() {
            return Rect::from_min_size(Pos2::new(PAD, PAD), Vec2::new(self.box_w, self.box_h));
        }
        let s = snap_flush(&self.mon_rects[b.monitor_index], b.edge, self.box_w as i32, self.box_h as i32);
        Rect::from_min_size(Pos2::new(s.left as f32, s.top as f32), Vec2::new(s.width as f32, s.height as f32))
    }

    /// 그리고 끌기를 처리한다. 배치가 바뀌면 Some(새 배치).
    pub fn show(
        &mut self,
        ui: &Ui,
        origin: Pos2,
        viewport: Vec2,
        pal: &'static Palette,
        off: i32,
        interactive: bool,
        label: &dyn Fn(i32) -> String,
        this_pc: Option<String>,
    ) -> Option<Vec<EdgeAttachment>> {
        self.sub_h = ui.painter().layout_no_wrap("가".into(), font(10.0, off, false), Color32::WHITE).size().y;
        self.relayout(viewport);
        self.recompute_monitor_rects();
        let host = Rect::from_min_size(origin, viewport);
        let p = ui.painter().with_clip_rect(host.intersect(ui.clip_rect()));
        let at = |r: Rect| r.translate(origin.to_vec2());

        p.rect_filled(host, 0, c(pal.canvas_bg));
        // 격자 22px.
        let grid = Stroke::new(1.0, c(pal.canvas_grid));
        let mut x = 0.0;
        while x < self.size.x {
            p.line_segment([origin + Vec2::new(x + 0.5, 0.0), origin + Vec2::new(x + 0.5, self.size.y)], grid);
            x += 22.0;
        }
        let mut y = 0.0;
        while y < self.size.y {
            p.line_segment([origin + Vec2::new(0.0, y + 0.5), origin + Vec2::new(self.size.x, y + 0.5)], grid);
            y += 22.0;
        }

        // 모니터(이 PC). 누를 수 없는 것이라 파랑을 쓰지 않는다. 글자는 칸에 들어가는 줄만(넘치면 그 줄 생략).
        let multi = self.mon_rects.len() > 1;
        for (i, m) in self.mon_rects.iter().enumerate() {
            let rect = at(Rect::from_min_size(
                Pos2::new(m.left as f32, m.top as f32),
                Vec2::new(m.width as f32, m.height as f32),
            ));
            let primary = i == self.primary;
            p.rect_filled(rect, CornerRadius::same(5), c(pal.mon_fill));
            p.rect_stroke(rect, CornerRadius::same(5), Stroke::new(1.5, c(pal.mon_stroke)), StrokeKind::Middle);
            let mut lines: Vec<(String, bool)> = vec![(format!("모니터 {}", i + 1), true)];
            if let Some(t) = &this_pc {
                lines.push((t.clone(), false));
            }
            if multi && primary {
                lines.push(("주 모니터".into(), false));
            }
            let galleys: Vec<_> = lines
                .into_iter()
                .map(|(t, bold)| p.layout_no_wrap(t, font(10.5, off, bold), c(if bold { pal.ink } else { pal.muted })))
                .filter(|g| g.size().x <= rect.width() - 6.0)
                .collect();
            let mut shown = Vec::new();
            let mut total = 0.0;
            for g in galleys {
                if total + g.size().y > rect.height() - 4.0 {
                    break;
                }
                total += g.size().y;
                shown.push(g);
            }
            let mut ty = rect.center().y - total / 2.0;
            for g in shown {
                let h = g.size().y;
                p.galley(Pos2::new(rect.center().x - g.size().x / 2.0, ty), g, c(pal.ink));
                ty += h;
            }
        }

        // 채널 박스.
        let mut tray = 0;
        for i in 0..self.boxes.len() {
            let placed = self.boxes[i].placed;
            let r = self.display_rect(i, if placed { 0 } else { tray });
            if !placed && Some(i) != self.dragging {
                tray += 1;
            }
            let r = at(r);
            let auto = placed;
            p.rect_filled(r, CornerRadius::same(5), c(if auto { pal.pc_auto_fill() } else { pal.pc_manual_fill }));
            let stroke_col = c(if auto { pal.accent } else { pal.pc_manual_stroke });
            if auto {
                p.rect_stroke(r, CornerRadius::same(5), Stroke::new(1.5, stroke_col), StrokeKind::Middle);
            } else {
                let mut pts = rounded_points(r, 5.0);
                pts.push(pts[0]);
                p.extend(Shape::dashed_line(&pts, Stroke::new(1.5, stroke_col), 4.5, 1.5));
            }
            let gs = r.width().min(r.height()) * 0.34;
            draw_icon(
                &p,
                Icon::Monitor,
                Rect::from_center_size(r.center(), Vec2::splat(gs)),
                c(if auto { pal.accent } else { pal.muted }),
            );
            let title = label(self.boxes[i].channel);
            let ft = font(11.0, off, true);
            let th = p.layout_no_wrap(title.clone(), ft.clone(), Color32::WHITE).size().y;
            p.text(Pos2::new(r.left(), r.top() - th - 2.0), Align2::LEFT_TOP, title, ft, c(pal.ink));
            p.text(
                Pos2::new(r.left(), r.bottom() + 2.0),
                Align2::LEFT_TOP,
                if auto { "붙음 · 자동" } else { "떨어짐 · 수동" },
                font(10.0, off, false),
                c(pal.muted),
            );
        }

        if !interactive {
            return None;
        }
        let resp = ui.interact(host, Id::unique("lf-canvas"), Sense::click_and_drag());
        let local = |pos: Pos2| pos - origin.to_vec2();
        // 끌 수 있다는 단서 — 상자 위에선 손(잡기), 끄는 중엔 쥔 손.
        if self.dragging.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if let Some(hp) = resp.hover_pos() {
            let lp = local(hp);
            let mut tray_i = 0;
            for i in 0..self.boxes.len() {
                let placed = self.boxes[i].placed;
                if self.display_rect(i, if placed { 0 } else { tray_i }).contains(lp) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    break;
                }
                if !placed {
                    tray_i += 1;
                }
            }
        }
        if resp.drag_started() {
            if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                let lp = local(press);
                for i in (0..self.boxes.len()).rev() {
                    let ti = self.boxes[..i].iter().filter(|b| !b.placed).count();
                    let r = self.display_rect(i, ti);
                    if r.contains(lp) {
                        self.dragging = Some(i);
                        self.drag_offset = lp - r.min;
                        self.boxes[i].rect = Some(r);
                        break;
                    }
                }
            }
        }
        if let (Some(i), Some(pos)) = (self.dragging, resp.interact_pointer_pos()) {
            if resp.dragged() {
                let lp = local(pos);
                // 칸(격자) 밖으로 못 나가게 — 보이지 않는 곳으로 사라지는 것 방지.
                let x = (lp.x - self.drag_offset.x).clamp(0.0, (self.size.x - self.box_w).max(0.0));
                let y = (lp.y - self.drag_offset.y).clamp(0.0, (self.size.y - self.box_h).max(0.0));
                self.boxes[i].rect = Some(Rect::from_min_size(Pos2::new(x, y), Vec2::new(self.box_w, self.box_h)));
                ui.ctx().request_repaint();
            }
        }
        if resp.drag_stopped() {
            if let Some(i) = self.dragging.take() {
                let r = self.boxes[i].rect.unwrap_or(Rect::NOTHING);
                let br = HandoffRect::new(r.left() as i32, r.top() as i32, self.box_w as i32, self.box_h as i32);
                match resolve_attachment(&self.mon_rects, &br, SNAP_PX) {
                    Some((idx, edge)) => {
                        self.boxes[i].placed = true;
                        self.boxes[i].monitor_index = idx;
                        self.boxes[i].edge = edge;
                    }
                    None => self.boxes[i].placed = false, // 아래 줄로 돌려보냄(분실 방지)
                }
                self.boxes[i].rect = None;
                return Some(self.attachments());
            }
        }
        None
    }
}
