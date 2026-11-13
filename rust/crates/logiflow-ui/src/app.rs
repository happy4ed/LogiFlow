//! 설정 창 — C# `MainForm`(사이드바 + 전환 관리·설정·진단 3화면)과 대화상자(진행 모달·라이선스)를
//! 같은 좌표·같은 문구로. 상태의 주인은 트레이 — 여기서는 보기와 편집 요청만(IPC).

use crate::canvas::{Canvas, Screen};
use crate::paint::{c, nav_button, thumb_color, Icon, Pc, Variant};
use egui::{Align2, Color32, CornerRadius, Event, Id, MouseWheelUnit, Pos2, Rect, Sense, Ui, Vec2, ViewportBuilder, ViewportCommand, ViewportId};
use logiflow_core::config::{AppConfig, ThemeMode};
use logiflow_core::scan::{self, looks_like_keyboard, ScanResult};
use logiflow_core::theme::Palette;
use logiflow_win::util::message_box;
use logiflow_win::wnd::Hwnd;
use logiflow_win::{clipboard, firewall, ipc, legacy, monitors, notify, prelogon, probe, prompt, theme_sys, tray};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const CONTENT_MAX: f32 = 560.0;
const SIDEBAR_W: f32 = 182.0;
const BAR_W: f32 = 10.0;

/// 트레이에서 받은 상태(IPC 창 → 여기).
#[derive(Default)]
pub struct Shared {
    pub config: AppConfig,
    pub devices: Vec<ScanResult>,
    pub scanning: bool,
    pub clip_running: bool,
    pub switch_result: Option<bool>,
    pub activate: bool,
}

struct Modal {
    title: String,
    step: Arc<Mutex<String>>,
    finished: Arc<Mutex<Option<(String, bool)>>>,
    /// 끝난 뒤 설정에 반영할 「로그온 전에도 실행」 값.
    pre_logon_after: Arc<Mutex<Option<bool>>>,
    started: Instant,
}

pub struct SettingsApp {
    pub shared: Arc<Mutex<Shared>>,
    pub tray: Option<Hwnd>,
    hwnd: Option<HWND>,
    page: usize,
    offset: f32,
    content_h: f32,
    thumb_drag: bool,
    show_all: bool,
    prelogon_installed: Arc<Mutex<bool>>,
    canvas: Canvas,
    modal: Option<Modal>,
    license: bool,
    license_offset: f32,
    busy: bool,
    dark_cache: (bool, Option<Instant>),
    first: bool,
    version: String,
    /// CI 화면 대조용(`--ui-snapshot=<폴더>`). 실사용 경로에는 없음.
    pub snapshot: Option<Snapshot>,
}

/// 화면 대조 캡처 계획 — (화면, 다크, 글자 단계, 아래로 스크롤) 마다 한 장.
pub struct Snapshot {
    pub dir: std::path::PathBuf,
    pub steps: Vec<(usize, bool, i32, bool)>,
    pub idx: usize,
    pub frames: u32,
    pub requested: bool,
    pub done: Arc<std::sync::atomic::AtomicBool>,
}

impl SettingsApp {
    pub fn new(shared: Arc<Mutex<Shared>>, tray: Option<Hwnd>, version: String) -> Self {
        let installed = Arc::new(Mutex::new(false));
        {
            // 로그온 전 서비스 존재를 백그라운드로 1회 조회(설정 탭 지연 방지).
            let installed = installed.clone();
            std::thread::spawn(move || {
                let v = prelogon::is_installed();
                *installed.lock().unwrap() = v;
            });
        }
        SettingsApp {
            shared,
            tray,
            hwnd: None,
            page: 0,
            offset: 0.0,
            content_h: 0.0,
            thumb_drag: false,
            show_all: false,
            prelogon_installed: installed,
            canvas: Canvas::default(),
            modal: None,
            license: false,
            license_offset: 0.0,
            busy: false,
            dark_cache: (false, None),
            first: true,
            version,
            snapshot: None,
        }
    }

    /// 설정을 바꾸고 트레이에 적용 요청(C# _tray.ApplyConfig()).
    fn apply(&mut self, f: impl FnOnce(&mut AppConfig)) {
        let text = {
            let mut s = self.shared.lock().unwrap();
            f(&mut s.config);
            s.config.serialize(None)
        };
        if let Some(t) = self.tray {
            ipc::send(t.get(), ipc::SET_CONFIG, &text, 2000);
        }
    }

    fn dark(&mut self, mode: ThemeMode) -> bool {
        match mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => {
                // 레지스트리는 1초마다만.
                let stale = self.dark_cache.1.map(|t| t.elapsed().as_millis() > 1000).unwrap_or(true);
                if stale {
                    self.dark_cache = (theme_sys::system_is_dark(), Some(Instant::now()));
                }
                self.dark_cache.0
            }
        }
    }

    fn owner(&self) -> Option<HWND> {
        self.hwnd
    }

    fn msg(&self, text: &str, title: &str, style: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
        message_box(self.owner(), text, title, style)
    }

    /// egui 좌표(point) → 화면 물리 좌표(메뉴 위치).
    fn screen_point(&self, ctx: &egui::Context, p: Pos2) -> (i32, i32) {
        let ppp = ctx.pixels_per_point();
        let mut origin = POINT::default();
        if let Some(h) = self.hwnd {
            unsafe {
                let _ = ClientToScreen(h, &mut origin);
            }
        }
        (origin.x + (p.x * ppp) as i32, origin.y + (p.y * ppp) as i32)
    }

    fn menu(&self, ctx: &egui::Context, anchor_bottom_left: Pos2, items: &[notify::MenuItem]) -> u32 {
        let (x, y) = self.screen_point(ctx, anchor_bottom_left);
        match self.hwnd {
            Some(h) => notify::popup_menu(h, items, x, y, true),
            None => 0,
        }
    }

    fn rescan(&self) {
        if let Some(t) = self.tray {
            ipc::send(t.get(), ipc::RESCAN, "", 1000);
        }
    }
}

fn switchable(devs: &[ScanResult]) -> Vec<ScanResult> {
    scan::switchable(devs)
}
fn current_channel(devs: &[ScanResult]) -> i32 {
    switchable(devs).iter().find(|r| r.current_host >= 0).map(|r| r.current_host).unwrap_or(-1)
}
fn max_host_count(devs: &[ScanResult]) -> i32 {
    switchable(devs).iter().map(|r| r.host_count).max().unwrap_or(0).max(0)
}
fn dwell_text(ms: i32) -> String {
    let s = format!("{:.2}", ms as f64 / 1000.0);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{}초", s)
}
fn cw(vw: f32) -> f32 {
    (vw - 48.0).min(CONTENT_MAX)
}

const LICENSE_BODY: &str = "LogiFlow\r\n로지텍 Easy-Switch 모니터 가장자리 자동 전환\r\n\r\nMIT License\r\n\r\nCopyright (c) 2026 happy4ed\r\n\r\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the \"Software\"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:\r\n\r\nThe above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.\r\n\r\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.\r\n\r\n─────────────────────────────\r\n구성 / 사용 기술\r\n• Rust · egui / eframe · wgpu (MIT / Apache-2.0)\r\n• windows-rs (Microsoft, MIT / Apache-2.0) — Windows HID / SetupAPI — Logitech HID++ 2.0 (feature 0x1814 Change Host)\r\n• windows-service · arboard · png (MIT / Apache-2.0)\r\n• RustCrypto aes · cbc · hmac · sha1 · sha2 · pbkdf2 (MIT / Apache-2.0)\r\n• chrono · unicode-normalization · getrandom · subtle (MIT / Apache-2.0 · BSD-3-Clause)\r\n• 설치 패키지: Inno Setup\r\n";

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.first {
            self.first = false;
            self.hwnd = crate::window_hwnd(frame);
            if let Some(h) = self.hwnd {
                crate::set_window_icon(h);
                if let Some(t) = self.tray {
                    ipc::send(t.get(), ipc::HELLO, &ipc::hwnd_text(crate::ipc_window()), 2000);
                }
                let _ = h;
            }
        }
        // 트레이가 보낸 ACTIVATE · 전환 결과.
        let (activate, switch_result) = {
            let mut s = self.shared.lock().unwrap();
            (std::mem::take(&mut s.activate), s.switch_result.take())
        };
        if activate {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        if let Some(ok) = switch_result {
            self.busy = false;
            self.msg(
                if ok {
                    "전환 명령을 보냈습니다."
                } else {
                    "전환 실패 — 로그를 확인하세요."
                },
                "LogiFlow",
                MB_OK | MB_ICONINFORMATION,
            );
        }

        let (cfg, devices, scanning, clip_running) = {
            let s = self.shared.lock().unwrap();
            (s.config.clone(), s.devices.clone(), s.scanning, s.clip_running)
        };
        let pal = Palette::of(self.dark(cfg.theme));
        let off = cfg.font_offset;
        let full = ui.max_rect();
        let interactive = self.modal.is_none() && !self.license;
        ui.painter().rect_filled(full, 0, c(pal.content));

        // ---- 사이드바 ----
        let sb = Pc {
            ui,
            pal,
            off,
            origin: full.min,
            interactive,
            id_base: Id::unique("sidebar"),
        };
        sb.fill(0.0, 0.0, SIDEBAR_W, full.height(), pal.sidebar);
        sb.icon(Icon::Mark, 16.0, 23.0, 18.0, pal.accent);
        sb.text(42.0, 22.0, "LogiFlow", 10.0, true, pal.ink);
        let nav = [("전환 관리", Icon::Swap), ("설정", Icon::Gear), ("진단", Icon::Diag)];
        for (i, (t, ic)) in nav.iter().enumerate() {
            let y = 62.0 + i as f32 * 40.0;
            if nav_button(&sb, &format!("nav{}", i), 12.0, y, 158.0, 36.0, t, *ic, self.page == i) && self.page != i {
                self.page = i;
                self.offset = 0.0; // C#: 화면 바꾸면 맨 위로
            }
        }
        let h = full.height();
        let status_y = h - 12.0 - 20.0 - 32.0;
        sb.icon(Icon::Plug, 18.0, status_y + 8.0, 16.0, pal.muted);
        sb.icon(Icon::Dot, 38.0, status_y + 8.0, 16.0, pal.ok);
        sb.text(54.0, status_y + 7.0, "LogiFlow 실행 중", 8.5, false, pal.muted);
        sb.text(20.0, h - 32.0 + 3.0, &format!("v{}", self.version), 8.0, false, pal.muted);

        // ---- 페이지(테마 스크롤) ----
        let host = Rect::from_min_max(Pos2::new(full.left() + SIDEBAR_W, full.top()), full.max);
        let vw = (host.width() - BAR_W).max(1.0);
        let page_w = vw.max(500.0);
        // 휠 스크롤(WinForms: 한 칸 = 120px).
        let hovered = ui.rect_contains_pointer(host);
        if hovered && interactive {
            let mut dy = 0.0;
            ui.input(|i| {
                for e in &i.events {
                    if let Event::MouseWheel { unit, delta, .. } = e {
                        dy += match unit {
                            MouseWheelUnit::Line => delta.y * 120.0,
                            MouseWheelUnit::Point => delta.y,
                            MouseWheelUnit::Page => delta.y * host.height(),
                        };
                    }
                }
            });
            self.offset -= dy;
        }
        let max_off = (self.content_h - host.height()).max(0.0);
        self.offset = self.offset.clamp(0.0, max_off);

        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(host));
        child.set_clip_rect(Rect::from_min_max(host.min, Pos2::new(host.left() + vw, host.bottom())).intersect(ui.clip_rect()));
        let origin = Pos2::new(host.left(), host.top() - self.offset);
        let content_h = {
            let pc = Pc {
                ui: &child,
                pal,
                off,
                origin,
                interactive,
                id_base: Id::unique(("page", self.page)),
            };
            match self.page {
                1 => self.settings_page(&pc, &ctx, page_w, &cfg, clip_running),
                2 => self.diag_page(&pc, &ctx, page_w, &devices, scanning),
                _ => self.switch_page(&pc, &ctx, page_w, &cfg, &devices),
            }
        };
        self.content_h = content_h;

        // 스크롤바(자리는 늘 예약, 넘칠 때만 손잡이).
        if content_h > host.height() {
            let bar = Rect::from_min_size(Pos2::new(host.right() - BAR_W, host.top()), Vec2::new(BAR_W, host.height()));
            let track = bar.height() - 4.0;
            let th = (host.height() / content_h * track).max(28.0);
            let max_thumb = track - th;
            let top = if max_off > 0.0 { self.offset / max_off * max_thumb } else { 0.0 };
            let thumb = Rect::from_min_size(Pos2::new(bar.left() + 2.0, bar.top() + 2.0 + top), Vec2::new(BAR_W - 4.0, th));
            let resp = ui.interact(bar, Id::unique("scrollbar"), if interactive { Sense::drag() } else { Sense::hover() });
            if resp.drag_started() {
                self.thumb_drag = ui.input(|i| i.pointer.press_origin()).map(|p| thumb.contains(p)).unwrap_or(false);
            }
            if resp.dragged() && self.thumb_drag && max_thumb > 0.0 {
                self.offset = (self.offset + resp.drag_delta().y / max_thumb * max_off).clamp(0.0, max_off);
            }
            if resp.drag_stopped() {
                self.thumb_drag = false;
            }
            let hot = resp.hovered() || self.thumb_drag;
            ui.painter().rect_filled(thumb, CornerRadius::same(3), thumb_color(pal, hot));
        }

        self.show_modal(&ctx, pal, off, full);
        self.show_license(&ctx, pal, off, full);
        self.snapshot_step(ui);
        if ui.input(|i| i.viewport().close_requested()) {
            if let Some(t) = self.tray {
                ipc::send(t.get(), ipc::BYE, "", 500);
            }
        }
    }
}

impl SettingsApp {
    // ================= 전환 관리 =================
    fn switch_page(&mut self, pc: &Pc<'_>, ctx: &egui::Context, vw: f32, cfg: &AppConfig, devices: &[ScanResult]) -> f32 {
        let p = pc.pal;
        let w = cw(vw);
        let mut y = 20.0;
        pc.text(24.0, y, "전환 관리", 14.0, true, p.ink);
        pc.text(24.0, y + 28.0, "화면 끝으로 밀면 자동 전환 · 떨어뜨리면 수동", 9.0, false, p.muted);
        if pc.button("refresh", 24.0 + w - 32.0, y, 32.0, 30.0, "", Some(Icon::Refresh), Variant::Icon, true) {
            self.rescan();
        }
        y += 58.0;

        // 캔버스.
        let canvas_h = 300.0;
        let max_host = max_host_count(devices);
        let current = current_channel(devices);
        let channels: Vec<i32> = (0..max_host).filter(|h| *h != current).collect();
        let screens: Vec<Screen> = monitors::all()
            .into_iter()
            .map(|m| Screen {
                name: m.name,
                bounds: m.bounds,
                primary: m.primary,
            })
            .collect();
        self.canvas.sync(&channels, current, &cfg.attachments, screens);
        let alias = |ch: i32| cfg.alias_for(ch);
        if let Some(att) = self
            .canvas
            .show(pc.ui, pc.origin + Vec2::new(24.0, y), Vec2::new(w, canvas_h), p, pc.off, pc.interactive, &alias)
        {
            self.apply(|c| c.attachments = att);
        }
        y += canvas_h + 14.0;

        // 채널 별칭.
        pc.text(24.0, y, "채널 별칭   엣지에 둘 다른 PC 를 알아보기 쉽게", 9.5, true, p.ink);
        y += 24.0;
        let count = max_host.max(1);
        let gap = 8.0;
        let chip_w = ((w - (count - 1) as f32 * gap) / count as f32).floor().max(110.0);
        let mut ax = 24.0;
        for ch in 0..count {
            pc.card(ax, y, chip_w, 32.0);
            let lab = format!("채널 {}", ch + 1);
            let lab_w = pc.measure_gdi(&lab, 8.0, true);
            pc.text(ax + 10.0, y + 8.0, &lab, 8.0, true, p.accent);
            let pen_x = ax + chip_w - 24.0 - 8.0;
            let al_left = ax + 10.0 + lab_w + 10.0;
            let al_w = (pen_x - al_left - 6.0).max(10.0);
            pc.text_ellipsis(al_left, y + 8.0, al_w, &cfg.alias_for(ch), 8.5, false, p.ink);
            let clicked_label = pc.click_area(&format!("al{}", ch), pc.r(al_left, y + 8.0, al_w, 18.0)).clicked() && pc.interactive;
            if pc.button(&format!("pen{}", ch), pen_x, y + 5.0, 24.0, 22.0, "", Some(Icon::Pencil), Variant::Icon, true) || clicked_label {
                self.edit_alias(ch, cfg, p);
            }
            ax += chip_w + gap;
        }
        y += 44.0;

        // 등록된 기기.
        let sw = switchable(devices);
        pc.text(24.0, y, &format!("등록된 기기   {}개", sw.len()), 9.5, true, p.ink);
        y += 26.0;
        if sw.is_empty() {
            pc.text(24.0, y, "전환 가능한 기기가 없습니다. [진단]에서 새로 고침 하세요.", 10.0, false, p.muted);
            y += 28.0;
        }
        for (i, d) in sw.iter().enumerate() {
            let ox = 24.0;
            pc.icon(
                if looks_like_keyboard(&d.device_name) { Icon::Keyboard } else { Icon::Mouse },
                ox + 2.0,
                y + 13.0,
                18.0,
                p.muted,
            );
            pc.text(ox + 30.0, y + 13.0, &d.device_name, 9.5, false, p.ink);
            let ch = if d.current_host >= 0 { (d.current_host + 1).to_string() } else { "?".into() };
            pc.badge(ox + w - 220.0, y + 10.0, 24.0, &format!("채널 {}", ch), false);
            if pc.button(&format!("sw{}", i), ox + w - 72.0, y + 9.0, 64.0, 26.0, "전환", None, Variant::Default, true) {
                let anchor = pc.r(ox + w - 72.0, y + 9.0, 64.0, 26.0).left_bottom();
                self.device_switch_menu(ctx, anchor, d, max_host, cfg);
            }
            pc.hline(ox, y + 43.0, w);
            y += 44.0;
        }
        y += 8.0;

        // footer
        let fx = 24.0;
        pc.hline(fx, y, w);
        pc.icon(Icon::Edge, fx, y + 16.0, 16.0, p.muted);
        pc.text(fx + 24.0, y + 15.0, "가장자리 자동 전환", 9.5, true, p.ink);
        let toggle_x = fx + w - 40.0;
        let dwell_x = toggle_x - 90.0;
        pc.text(dwell_x - 30.0, y + 18.0, "대기", 8.5, false, p.muted);
        if pc.button("dwell", dwell_x, y + 14.0, 78.0, 26.0, &dwell_text(cfg.dwell_ms), None, Variant::Default, true) {
            let anchor = pc.r(dwell_x, y + 14.0, 78.0, 26.0).left_bottom();
            let opts = [200, 330, 500, 800, 1000];
            let items: Vec<notify::MenuItem> = opts
                .iter()
                .enumerate()
                .map(|(i, ms)| notify::MenuItem::Item {
                    id: 100 + i as u32,
                    text: dwell_text(*ms),
                    enabled: true,
                })
                .collect();
            let r = self.menu(ctx, anchor, &items);
            if (100..105).contains(&r) {
                let v = opts[(r - 100) as usize];
                self.apply(|c| c.dwell_ms = v);
            }
        }
        if pc.toggle("edge", toggle_x, y + 20.0, cfg.edge_switch_enabled) {
            self.apply(|c| c.edge_switch_enabled = !c.edge_switch_enabled);
        }
        if pc.toggle("guard", fx, y + 52.0, cfg.drag_guard) {
            self.apply(|c| c.drag_guard = !c.drag_guard);
        }
        pc.text(fx + 50.0, y + 56.0, "드래그 중 전환 안 함", 8.5, false, p.ink);
        if pc.button("reset", fx + w - 90.0, y + 54.0, 90.0, 26.0, "배치 초기화", None, Variant::Ghost, true) {
            let att = self.canvas.clear_placements();
            self.apply(|c| c.attachments = att);
        }
        y += 86.0 + 20.0;
        y
    }

    fn edit_alias(&mut self, ch: i32, cfg: &AppConfig, p: &Palette) {
        let Some(h) = self.hwnd else { return };
        let cur = cfg.alias_for(ch);
        if let Some(v) = prompt::show(h, "채널 별칭", &format!("채널 {} 의 별칭", ch + 1), &cur, p.content, p.ink) {
            self.apply(|c| c.set_alias(ch, &v));
        }
    }

    fn device_switch_menu(&mut self, ctx: &egui::Context, anchor: Pos2, d: &ScanResult, max_host: i32, cfg: &AppConfig) {
        let hosts = if d.host_count > 0 { d.host_count } else { max_host };
        let mut items = Vec::new();
        for h in 0..hosts {
            if h == d.current_host {
                continue;
            }
            items.push(notify::MenuItem::Item {
                id: 200 + h as u32,
                text: format!("채널 {} · {}", h + 1, cfg.alias_for(h)),
                enabled: true,
            });
        }
        if items.is_empty() {
            items.push(notify::MenuItem::Item {
                id: 199,
                text: "전환할 다른 채널 없음".into(),
                enabled: false,
            });
        }
        let r = self.menu(ctx, anchor, &items);
        if (200..216).contains(&r) {
            let target = (r - 200) as i32;
            if self.busy {
                return;
            }
            let q = format!(
                "{} 를 채널 {} ({}) 로 전환합니다.\r\n이 PC 에서 기기가 사라집니다. 진행할까요?",
                d.device_name,
                target + 1,
                cfg.alias_for(target)
            );
            if self.msg(&q, "LogiFlow 전환", MB_YESNO | MB_ICONWARNING) != IDYES {
                return;
            }
            self.busy = true;
            if let Some(t) = self.tray {
                ipc::send(t.get(), ipc::SWITCH_ONE, &format!("{}\t{}\t{}", d.path, d.device_index, target), 2000);
            }
        }
    }

    // ================= 설정 =================
    fn settings_page(&mut self, pc: &Pc<'_>, ctx: &egui::Context, vw: f32, cfg: &AppConfig, clip_running: bool) -> f32 {
        let _ = ctx;
        let p = pc.pal;
        let w = cw(vw);
        let mut y = 20.0;
        pc.text(24.0, y, "설정", 14.0, true, p.ink);
        pc.text(24.0, y + 28.0, "실행과 로그를 관리", 9.0, false, p.muted);
        y += 60.0;

        // 모양
        pc.text(24.0, y, "모양", 8.0, true, p.muted);
        y += 22.0;
        let cx = 24.0;
        pc.card(cx, y, w, 132.0);
        row_label(pc, cx + 16.0, y + 16.0, "테마", "시스템 설정을 따르거나 라이트·다크로 고정합니다.");
        if let Some(i) = pc.segmented(
            "theme",
            cx + w - 246.0,
            y + 17.0,
            224.0,
            34.0,
            &["시스템", "라이트", "다크"],
            cfg.theme as usize,
        ) {
            self.apply(|c| c.theme = ThemeMode::from_i32(i as i32).unwrap_or_default());
        }
        pc.hline(cx + 16.0, y + 66.0, w - 32.0);
        row_label(
            pc,
            cx + 16.0,
            y + 82.0,
            "글자 크기",
            "보통이 기본입니다. 작은 글자가 안 보이면 크게로 키우세요.",
        );
        let font_idx = if cfg.font_offset >= 3 {
            2
        } else if cfg.font_offset <= -1 {
            0
        } else {
            1
        };
        if let Some(i) = pc.segmented("font", cx + w - 246.0, y + 83.0, 224.0, 34.0, &["작게", "보통", "크게"], font_idx) {
            let off = match i {
                2 => 3,
                0 => -1,
                _ => 1,
            };
            self.apply(|c| c.font_offset = off);
        }
        y += 132.0 + 16.0;

        // 실행
        pc.text(24.0, y, "실행", 8.0, true, p.muted);
        y += 22.0;
        pc.card(cx, y, w, 120.0);
        if setting_row(
            pc,
            "autostart",
            cx,
            y,
            w,
            0.0,
            "Windows 시작 시 실행",
            "로그인하면 트레이에 자동으로 상주합니다.",
            false,
            cfg.auto_start,
        ) {
            self.apply(|c| c.auto_start = !c.auto_start);
        }
        let installed = *self.prelogon_installed.lock().unwrap();
        if setting_row(
            pc,
            "prelogon",
            cx,
            y,
            w,
            60.0,
            "로그온 전에도 실행",
            "로그인/잠금 화면에서도 가장자리로 밀면 전환됩니다(부팅 시 SYSTEM 작업 · 관리자 권한). [전환 관리]에서 붙인 채널 배치를 씁니다.",
            true,
            installed,
        ) {
            self.toggle_prelogon(!installed, cfg);
        }
        y += 120.0 + 16.0;

        // 전환 · 진단
        pc.text(24.0, y, "전환 · 진단", 8.0, true, p.muted);
        y += 22.0;
        pc.card(cx, y, w, 180.0);
        if setting_row(
            pc,
            "edge2",
            cx,
            y,
            w,
            0.0,
            "가장자리 자동 전환 사용",
            "모니터 변에 붙인 채널로 커서를 밀면 자동 전환합니다.",
            false,
            cfg.edge_switch_enabled,
        ) {
            self.apply(|c| c.edge_switch_enabled = !c.edge_switch_enabled);
        }
        if setting_row(
            pc,
            "diaglog",
            cx,
            y,
            w,
            60.0,
            "진단 로그 남기기",
            "엣지 판정·전환 트리거를 로그 폴더에 기록합니다.",
            false,
            cfg.diag_log_enabled,
        ) {
            self.apply(|c| c.diag_log_enabled = !c.diag_log_enabled);
        }
        if setting_row(
            pc,
            "debug",
            cx,
            y,
            w,
            120.0,
            "상세 디버그 로그",
            "로그온 전환 워커가 커서·엣지별 거리·dwell 을 상세 기록(안 될 때 원인 추적). 켜면 몇 초 뒤 반영.",
            true,
            cfg.debug_mode,
        ) {
            self.apply(|c| c.debug_mode = !c.debug_mode);
        }
        y += 180.0 + 16.0;

        // 클립보드 공유
        pc.text(24.0, y, "클립보드 공유", 8.0, true, p.muted);
        y += 22.0;
        pc.card(cx, y, w, 240.0);
        if setting_row(
            pc,
            "clip",
            cx,
            y,
            w,
            0.0,
            "클립보드 공유 사용",
            "상대 PC 와 텍스트·이미지 클립보드를 주고받습니다(LAN 직접 연결).",
            false,
            cfg.clip_enabled,
        ) {
            self.apply(|c| c.clip_enabled = !c.clip_enabled);
        }
        let host_v = if cfg.clip_peer_host.is_empty() {
            "미설정".to_string()
        } else {
            cfg.clip_peer_host.clone()
        };
        if value_row(
            pc,
            "peer",
            cx,
            y,
            w,
            60.0,
            "상대 PC 주소",
            &format!("포트 {} · 양쪽 방화벽 허용 필요", cfg.clip_port),
            &host_v,
        ) {
            if let Some(h) = self.hwnd {
                if let Some(v) = prompt::show(h, "상대 PC 주소", "상대 PC 의 IP 또는 호스트명", &cfg.clip_peer_host, p.content, p.ink) {
                    let v = logiflow_core::clip::norm(&v);
                    self.apply(|c| c.clip_peer_host = v);
                }
            }
        }
        let sec_v = if cfg.clip_secret.is_empty() { "미설정" } else { "설정됨" };
        if value_row(pc, "secret", cx, y, w, 120.0, "암호문구", "양쪽 PC 에 같은 값 — 틀리면 전달 안 됨", sec_v) {
            if let Some(h) = self.hwnd {
                if let Some(v) = prompt::show(h, "암호문구", "양쪽 PC 동일하게 입력 (비우면 끔)", &cfg.clip_secret, p.content, p.ink) {
                    let v = logiflow_core::clip::norm(&v);
                    self.apply(|c| c.clip_secret = v);
                }
            }
        }
        // 진단 행
        let dy = y + 180.0;
        pc.hline(cx + 16.0, dy, w - 32.0);
        pc.text_box(
            cx + 16.0,
            dy + 7.0,
            w - 32.0,
            16.0,
            &format!("이 PC 주소(상대에 입력): {}", clipboard::local_info(cfg.clip_port)),
            8.5,
            false,
            p.muted,
        );
        if pc.button("ctest", cx + 16.0, dy + 28.0, 92.0, 26.0, "연결 테스트", None, Variant::Default, true) {
            let r = clipboard::test_connect(&cfg.clip_peer_host, cfg.clip_port);
            self.msg(&r, "연결 테스트", MB_OK | MB_ICONINFORMATION);
        }
        if pc.button("csend", cx + 116.0, dy + 28.0, 104.0, 26.0, "테스트 보내기", None, Variant::Default, true) {
            let r = clipboard::send_test(clip_running, cfg);
            self.msg(&r, "테스트 보내기", MB_OK | MB_ICONINFORMATION);
        }
        if pc.button("cfw", cx + 228.0, dy + 28.0, 96.0, 26.0, "방화벽 허용", None, Variant::Default, true) {
            let ok = firewall::request_allow(cfg.clip_port);
            let t = if ok {
                format!("방화벽 인바운드 허용 규칙을 추가했습니다(포트 {}).", cfg.clip_port)
            } else {
                "실패/취소 — 관리자 권한이 필요하고 회사 정책이 막을 수 있습니다.".to_string()
            };
            self.msg(&t, "방화벽 허용", MB_OK | if ok { MB_ICONINFORMATION } else { MB_ICONWARNING });
        }
        y += 240.0 + 20.0;

        // 정보
        pc.text(24.0, y, "정보", 8.0, true, p.muted);
        y += 22.0;
        pc.card(cx, y, w, 60.0);
        pc.text(cx + 16.0, y + 12.0, "오픈소스 라이선스", 9.5, true, p.ink);
        pc.text_box(
            cx + 16.0,
            y + 32.0,
            w - 120.0,
            18.0,
            "LogiFlow 와 사용된 구성요소의 라이선스(MIT).",
            8.5,
            false,
            p.muted,
        );
        if pc.button("lic", cx + w - 80.0, y + 17.0, 64.0, 26.0, "보기", None, Variant::Default, true) {
            self.license = true;
            self.license_offset = 0.0;
        }
        y += 60.0 + 20.0;
        y
    }

    fn toggle_prelogon(&mut self, on: bool, cfg: &AppConfig) {
        if on {
            if cfg.attachments.is_empty() {
                self.msg(
                    "먼저 [전환 관리]에서 모니터 변에 채널을 붙여 주세요. 그 배치를 잠금/로그인 화면에서도 씁니다.",
                    "LogiFlow",
                    MB_OK | MB_ICONINFORMATION,
                );
                return;
            }
            // 지금 판과 다른 점(시험판): C# 판 서비스와 함께 돌면 로그온 화면에서 두 번 전환된다.
            if legacy::csharp_service_exists() {
                self.msg(
                    "기존 LogiFlow(C# 판)의 「로그온 전에도 실행」이 켜져 있습니다.\r\n두 판이 함께 전환하지 않도록, 기존 LogiFlow 에서 먼저 끈 뒤 다시 켜 주세요.",
                    "LogiFlow",
                    MB_OK | MB_ICONINFORMATION,
                );
                return;
            }
            let modal = Modal::new("로그온 전에도 실행 — 켜는 중");
            let (step, fin, after) = (modal.step.clone(), modal.finished.clone(), modal.pre_logon_after.clone());
            let cfg2 = cfg.clone();
            std::thread::spawn(move || {
                *step.lock().unwrap() = "채널 배치 저장 중…".into();
                tray::write_logon_edges(&cfg2); // 머신 공유 엣지 설정 먼저
                *step.lock().unwrap() = "관리자 권한 요청 → 서비스 등록 중…\r\nUAC 창이 뜨면 '예'를 눌러 주세요.".into();
                let ok = prelogon::request_enable(cfg2.my_host);
                *after.lock().unwrap() = Some(ok);
                *fin.lock().unwrap() = Some((
                    if ok {
                        "로그온 전에도 실행을 켰습니다.\r\n이제 콜드부팅·로그인·잠금 화면에서 가장자리로 밀면 전환됩니다(서비스 상주).\r\n\r\n※ 리시버/동글이 이 PC 에 있어야 하고, 콜드부팅 로그온 화면에선 마우스를 한 번 흔들어 깨운 뒤 미세요(키보드는 아무 키나 한 번).".into()
                    } else {
                        "켜기가 취소되었거나 실패했습니다.\r\n관리자 권한(UAC)이 필요하며, 회사 관리 PC 정책이 막을 수 있습니다.".into()
                    },
                    !ok,
                ));
            });
            self.modal = Some(modal);
        } else {
            let installed = *self.prelogon_installed.lock().unwrap();
            let had = installed || prelogon::is_installed();
            if !had {
                self.apply(|c| c.pre_logon = false);
                return;
            }
            let modal = Modal::new("로그온 전에도 실행 — 끄는 중");
            let (step, fin, after) = (modal.step.clone(), modal.finished.clone(), modal.pre_logon_after.clone());
            std::thread::spawn(move || {
                *step.lock().unwrap() = "관리자 권한 요청 → 서비스 제거 중…\r\nUAC 창이 뜨면 '예'를 눌러 주세요.".into();
                let ok_msg = prelogon::request_disable();
                tray::remove_logon_edges();
                // 삭제 성공을 낙관하지 말고 실제 서비스 상태를 재조회.
                let still = prelogon::is_installed();
                let good = ok_msg && !still;
                *after.lock().unwrap() = Some(still);
                *fin.lock().unwrap() = Some((
                    if good {
                        "로그온 전에도 실행을 껐습니다.".into()
                    } else {
                        "제거가 취소되었거나 실패했습니다(관리자 권한 필요).".into()
                    },
                    !good,
                ));
            });
            self.modal = Some(modal);
        }
    }

    fn show_modal(&mut self, ctx: &egui::Context, pal: &'static Palette, off: i32, parent: Rect) {
        let Some(m) = &self.modal else { return };
        let title = m.title.clone();
        let step = m.step.lock().unwrap().clone();
        let fin = m.finished.lock().unwrap().clone();
        let t = m.started.elapsed().as_secs_f32();
        let pos = center_on(ctx, parent, Vec2::new(440.0, 244.0));
        let mut close = false;
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("lf-progress"),
            ViewportBuilder::default()
                .with_title(title.clone())
                .with_inner_size([440.0, 244.0])
                .with_resizable(false)
                .with_close_button(false)
                .with_minimize_button(false)
                .with_maximize_button(false)
                .with_position(pos),
            |ui, _| {
                let r = ui.max_rect();
                ui.painter().rect_filled(r, 0, c(pal.content));
                let pc = Pc {
                    ui,
                    pal,
                    off,
                    origin: r.min,
                    interactive: true,
                    id_base: Id::unique("modal"),
                };
                pc.text_box(20.0, 16.0, 400.0, 24.0, &title, 10.5, true, pal.ink);
                match &fin {
                    None => {
                        pc.text_box(20.0, 48.0, 400.0, 150.0, &step, 9.0, false, pal.muted);
                        // 진행 막대(Marquee).
                        let bar = pc.r(20.0, 210.0, 400.0, 10.0);
                        ui.painter().rect_filled(bar, 0, Color32::from_gray(if pal.dark { 60 } else { 230 }));
                        let seg = 100.0;
                        let x = ((t * 160.0) % (400.0 + seg)) - seg;
                        let blk = Rect::from_min_size(
                            Pos2::new(bar.left() + x.max(0.0), bar.top()),
                            Vec2::new((seg + x.min(0.0)).min(400.0 - x.max(0.0)).max(0.0), 10.0),
                        );
                        ui.painter().rect_filled(blk, 0, Color32::from_rgb(6, 176, 37));
                        ui.ctx().request_repaint();
                    }
                    Some((text, _err)) => {
                        pc.text_box(20.0, 48.0, 400.0, 150.0, text, 9.0, false, pal.ink);
                        if pc.button("ok", 330.0, 202.0, 90.0, 32.0, "확인", None, Variant::Default, true) || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            close = true;
                        }
                    }
                }
            },
        );
        if close {
            let after = *self.modal.as_ref().unwrap().pre_logon_after.lock().unwrap();
            self.modal = None;
            if let Some(v) = after {
                *self.prelogon_installed.lock().unwrap() = v;
                self.apply(|c| c.pre_logon = v);
            }
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    fn show_license(&mut self, ctx: &egui::Context, pal: &'static Palette, off: i32, parent: Rect) {
        if !self.license {
            return;
        }
        let pos = center_on(ctx, parent, Vec2::new(560.0, 460.0));
        let mut close = false;
        let mut offset = self.license_offset;
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("lf-license"),
            ViewportBuilder::default()
                .with_title("오픈소스 라이선스")
                .with_inner_size([560.0, 460.0])
                .with_resizable(false)
                .with_minimize_button(false)
                .with_maximize_button(false)
                .with_position(pos),
            |ui, _| {
                if ui.input(|i| i.viewport().close_requested()) {
                    close = true;
                }
                let r = ui.max_rect();
                ui.painter().rect_filled(r, 0, c(pal.content));
                let pc = Pc {
                    ui,
                    pal,
                    off,
                    origin: r.min,
                    interactive: true,
                    id_base: Id::unique("license"),
                };
                // 본문(테마 스크롤, Field 바탕).
                let area = pc.r(16.0, 16.0, 528.0, 460.0 - 48.0 - 24.0);
                ui.painter().rect_filled(area, 0, c(pal.field));
                let text_w = area.width() - BAR_W - 12.0;
                let galley = ui.painter().layout(LICENSE_BODY.replace("\r\n", "\n"), pc.f(9.0, false), c(pal.ink), text_w);
                let content_h = galley.size().y + 16.0;
                if ui.rect_contains_pointer(area) {
                    ui.input(|i| {
                        for e in &i.events {
                            if let Event::MouseWheel { unit, delta, .. } = e {
                                offset -= match unit {
                                    MouseWheelUnit::Line => delta.y * 120.0,
                                    MouseWheelUnit::Point => delta.y,
                                    MouseWheelUnit::Page => delta.y * area.height(),
                                };
                            }
                        }
                    });
                }
                let max_off = (content_h - area.height()).max(0.0);
                offset = offset.clamp(0.0, max_off);
                ui.painter()
                    .with_clip_rect(area)
                    .galley(area.min + Vec2::new(6.0, 6.0 - offset), galley, c(pal.ink));
                if content_h > area.height() {
                    let track = area.height() - 4.0;
                    let th = (area.height() / content_h * track).max(28.0);
                    let top = if max_off > 0.0 { offset / max_off * (track - th) } else { 0.0 };
                    let thumb = Rect::from_min_size(Pos2::new(area.right() - BAR_W + 2.0, area.top() + 2.0 + top), Vec2::new(BAR_W - 4.0, th));
                    let resp = ui.interact(
                        Rect::from_min_size(Pos2::new(area.right() - BAR_W, area.top()), Vec2::new(BAR_W, area.height())),
                        Id::unique("lic-bar"),
                        Sense::drag(),
                    );
                    if resp.dragged() && track > th {
                        offset = (offset + resp.drag_delta().y / (track - th) * max_off).clamp(0.0, max_off);
                    }
                    ui.painter()
                        .rect_filled(thumb, CornerRadius::same(3), thumb_color(pal, resp.hovered() || resp.dragged()));
                }
                if pc.button("close", 560.0 - 104.0, 460.0 - 48.0 + 10.0, 88.0, 28.0, "닫기", None, Variant::Default, true)
                    || ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape))
                {
                    close = true;
                }
            },
        );
        self.license_offset = offset;
        if close {
            self.license = false;
        }
    }

    // ================= 진단 =================
    fn diag_page(&mut self, pc: &Pc<'_>, _ctx: &egui::Context, vw: f32, devices: &[ScanResult], scanning: bool) -> f32 {
        let p = pc.pal;
        let w = cw(vw);
        let mut y = 20.0;
        pc.text(24.0, y, "진단", 14.0, true, p.ink);
        pc.text(24.0, y + 28.0, "기기별 채널·전환 인덱스 확인", 9.0, false, p.muted);
        let log_x = 24.0 + w - 126.0;
        let ref_x = log_x - 10.0 - 108.0;
        let show_x = ref_x - 10.0 - 40.0;
        let lab_w = pc.measure_gdi("모두 보기", 9.0, false);
        pc.text(show_x - 6.0 - lab_w, y + 6.0, "모두 보기", 9.0, false, p.ink);
        if pc.toggle("showall", show_x, y + 3.0, self.show_all) {
            self.show_all = !self.show_all;
        }
        if pc.button(
            "refresh",
            ref_x,
            y,
            108.0,
            28.0,
            if scanning { "스캔 중…" } else { "새로 고침" },
            Some(Icon::Refresh),
            Variant::Default,
            !scanning,
        ) {
            self.rescan();
        }
        if pc.button("logdir", log_x, y, 126.0, 28.0, "로그 폴더 열기", Some(Icon::Folder), Variant::Default, true) {
            let dir = logiflow_win::util::exe_dir().join("logs");
            let _ = std::fs::create_dir_all(&dir);
            logiflow_win::util::open_with("explorer.exe", &dir);
        }
        y += 58.0;

        let shown: Vec<&ScanResult> = devices.iter().filter(|r| self.show_all || r.change_host_supported).collect();
        let sw = switchable(devices).len();
        pc.text(24.0, y, &format!("스캔된 기기   전환 가능 {}개", sw), 9.5, true, p.ink);
        y += 24.0;
        // 머리 줄
        let cols = [
            ("기기", 0.0, 0.30),
            ("VID:PID", 0.30, 0.18),
            ("연결", 0.48, 0.22),
            ("0x1814 인덱스", 0.70, 0.18),
            ("채널", 0.88, 0.12),
        ];
        for (t, x, cwid) in cols {
            pc.text_box(24.0 + (w * x).floor(), y + 4.0, (w * cwid).floor(), 18.0, t, 8.0, true, p.muted);
        }
        pc.hline(24.0, y + 25.0, w);
        y += 28.0;
        if shown.is_empty() {
            pc.text(24.0, y, "표시할 기기가 없습니다. [새로 고침] 하세요.", 10.0, false, p.muted);
            y += 30.0;
        }
        for r in shown {
            let ox = 24.0;
            pc.icon(
                if looks_like_keyboard(&r.device_name) { Icon::Keyboard } else { Icon::Mouse },
                ox,
                y + 11.0,
                17.0,
                p.muted,
            );
            pc.text_box(ox + 26.0, y + 11.0, (w * 0.30).floor() - 26.0, 18.0, &r.device_name, 9.0, false, p.ink);
            pc.text_box(ox + (w * 0.30).floor(), y + 11.0, (w * 0.18).floor(), 18.0, &r.vid_pid, 9.0, false, p.muted);
            pc.text_box(
                ox + (w * 0.48).floor(),
                y + 11.0,
                (w * 0.22).floor(),
                18.0,
                &r.connection_type,
                9.0,
                false,
                p.ink,
            );
            let idx = if r.change_host_supported {
                format!("0x{:02X}", r.change_host_index)
            } else {
                "미지원".into()
            };
            pc.text_box(ox + (w * 0.70).floor(), y + 11.0, (w * 0.18).floor(), 18.0, &idx, 9.0, false, p.ink);
            let ch = if r.current_host >= 0 {
                format!("채널 {}", r.current_host + 1)
            } else {
                "-".into()
            };
            pc.badge(ox + (w * 0.88).floor(), y + 8.0, 22.0, &ch, false);
            pc.hline(ox, y + 39.0, w);
            y += 40.0;
        }
        y += 6.0;
        pc.icon(Icon::Plug, 24.0, y + 1.0, 14.0, p.muted);
        pc.text(
            44.0,
            y,
            "기기 정보는 저장되어 매번 재검색하지 않음 · \"모두 보기\"를 켜면 빈 슬롯·리시버도 함께 표시",
            8.5,
            false,
            p.muted,
        );
        y += 40.0;

        pc.text(24.0, y, "로그온 화면 접근 테스트 (실험 · 읽기 전용)", 8.0, true, p.muted);
        y += 24.0;
        if pc.button("pinstall", 24.0, y, 140.0, 30.0, "테스트 설치·실행", None, Variant::Primary, true) {
            let ok = probe::request_install_run();
            self.msg(
                if ok {
                    "테스트를 설치하고 지금 SYSTEM 으로 실행했습니다(약 2분).\r\n\r\n① 바로 Win+L 로 잠금화면에 가서 마우스를 20~30초 움직이세요.\r\n② 로그인해 돌아와 [로그 열기]를 누르세요.\r\n③ desktop=\"Winlogon\" 줄의 GetCursorPos 가 성공이면 로그인 화면 전환 구현 가능입니다."
                } else {
                    "설치가 취소되었거나 실패했습니다.\r\n관리자 권한(UAC)이 필요하며, 회사 관리 PC 정책이 SYSTEM 작업 생성을 막으면 여기서 실패합니다(그 자체가 '정책상 불가' 판정)."
                },
                "LogiFlow 로그온 화면 테스트",
                MB_OK | if ok { MB_ICONINFORMATION } else { MB_ICONWARNING },
            );
        }
        if pc.button("popen", 24.0 + 148.0, y, 104.0, 30.0, "로그 열기", Some(Icon::Folder), Variant::Default, true) {
            probe::open_log(self.owner());
        }
        if pc.button("premove", 24.0 + 148.0 + 112.0, y, 70.0, 30.0, "제거", None, Variant::Ghost, true) {
            let ok = probe::request_uninstall();
            self.msg(
                if ok {
                    "테스트를 제거했습니다."
                } else {
                    "제거가 취소되었거나 실패했습니다."
                },
                "LogiFlow",
                MB_OK | if ok { MB_ICONINFORMATION } else { MB_ICONWARNING },
            );
        }
        y += 38.0;
        pc.text_box(
            24.0,
            y,
            w,
            54.0,
            "관리자 승인(UAC) 후 SYSTEM 으로 실행됩니다. 바로 Win+L 로 잠금화면에 가서 마우스를 20~30초 움직인 뒤 돌아와 [로그 열기]. desktop=\"Winlogon\" 줄의 GetCursorPos 가 성공(좌표가 바뀜)이면 로그인 화면 엣지 전환 구현이 가능합니다. 이 테스트는 읽기만 하고 마우스·키보드를 전환하지 않습니다.",
            8.5,
            false,
            p.muted,
        );
        y += 60.0;
        y
    }
}

impl SettingsApp {
    /// 캡처 계획 한 단계: 상태를 맞추고 몇 프레임 뒤 Screenshot 명령 → 받은 이미지를 PNG 로.
    fn snapshot_step(&mut self, ui: &Ui) {
        let Some(sn) = self.snapshot.as_mut() else { return };
        let ctx = ui.ctx().clone();
        if sn.requested && sn.done.load(std::sync::atomic::Ordering::SeqCst) {
            sn.idx += 1;
            sn.frames = 0;
            sn.requested = false;
            sn.done.store(false, std::sync::atomic::Ordering::SeqCst);
        }
        if sn.idx >= sn.steps.len() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        let (page, dark, off, bottom) = sn.steps[sn.idx];
        if sn.frames == 0 {
            self.page = page;
            self.offset = if bottom { 1.0e6 } else { 0.0 };
            let mut s = self.shared.lock().unwrap();
            s.config.theme = if dark { ThemeMode::Dark } else { ThemeMode::Light };
            s.config.font_offset = off;
        }
        let sn = self.snapshot.as_mut().unwrap();
        sn.frames += 1;
        if sn.frames == 4 && !sn.requested {
            sn.requested = true;
            let name = format!(
                "{}-{}-font{}{}.png",
                ["switch", "settings", "diag"][page],
                if dark { "dark" } else { "light" },
                off,
                if bottom { "-bottom" } else { "" }
            );
            let path = sn.dir.join(name);
            let _ = std::fs::create_dir_all(&sn.dir);
            let done = sn.done.clone();
            let ctx2 = ctx.clone();
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                if let Ok(f) = std::fs::File::create(&path) {
                    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), img.size[0] as u32, img.size[1] as u32);
                    enc.set_color(png::ColorType::Rgba);
                    enc.set_depth(png::BitDepth::Eight);
                    if let Ok(mut w) = enc.write_header() {
                        let bytes: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_array()).collect();
                        let _ = w.write_image_data(&bytes);
                    }
                }
                done.store(true, std::sync::atomic::Ordering::SeqCst);
                ctx2.request_repaint();
            })));
        }
        ctx.request_repaint();
    }
}

impl Modal {
    fn new(title: &str) -> Self {
        Modal {
            title: title.into(),
            step: Arc::new(Mutex::new("준비 중…".into())),
            finished: Arc::new(Mutex::new(None)),
            pre_logon_after: Arc::new(Mutex::new(None)),
            started: Instant::now(),
        }
    }
}

fn center_on(ctx: &egui::Context, parent: Rect, size: Vec2) -> Pos2 {
    let outer = ctx.input(|i| i.viewport().outer_rect).unwrap_or(parent);
    Pos2::new(outer.center().x - size.x / 2.0, outer.center().y - size.y / 2.0)
}

/// C# MakeRowLabel — 굵은 제목 + 두 줄 설명(288×32).
fn row_label(pc: &Pc<'_>, x: f32, y: f32, lab: &str, desc: &str) {
    pc.text(x, y, lab, 9.5, true, pc.pal.ink);
    pc.text_box(x, y + 19.0, 288.0, 32.0, desc, 8.5, false, pc.pal.muted);
}

/// C# AddSettingRow — 토글이 눌리면 true.
#[allow(clippy::too_many_arguments)]
fn setting_row(pc: &Pc<'_>, key: &str, cx: f32, cy: f32, w: f32, top: f32, lab: &str, desc: &str, advanced: bool, on: bool) -> bool {
    let y = cy + top;
    let lw = pc.text(cx + 16.0, y + 12.0, lab, 9.5, true, pc.pal.ink);
    if advanced {
        pc.badge(cx + 16.0 + lw + 8.0, y + 11.0, 18.0, "고급", true);
    }
    pc.text_box(cx + 16.0, y + 32.0, w - 90.0, 18.0, desc, 8.5, false, pc.pal.muted);
    if top > 0.0 {
        pc.hline(cx + 16.0, y, w - 32.0);
    }
    pc.toggle(key, cx + w - 56.0, y + 19.0, on)
}

/// C# AddValueRow — [편집] 이 눌리면 true.
#[allow(clippy::too_many_arguments)]
fn value_row(pc: &Pc<'_>, key: &str, cx: f32, cy: f32, w: f32, top: f32, lab: &str, desc: &str, value: &str) -> bool {
    let y = cy + top;
    pc.text(cx + 16.0, y + 12.0, lab, 9.5, true, pc.pal.ink);
    pc.text_box(cx + 16.0, y + 32.0, w - 180.0, 18.0, desc, 8.5, false, pc.pal.muted);
    let edit_x = cx + w - 80.0;
    let vw = pc.measure_gdi(value, 9.0, true);
    let vx = (cx + 120.0).max(edit_x - 12.0 - vw);
    pc.text(vx, y + 15.0, value, 9.0, true, pc.pal.accent);
    if top > 0.0 {
        pc.hline(cx + 16.0, y, w - 32.0);
    }
    pc.button(key, edit_x, y + 17.0, 64.0, 26.0, "편집", None, Variant::Default, true)
}

#[allow(dead_code)]
fn unused(_: Align2) {}
