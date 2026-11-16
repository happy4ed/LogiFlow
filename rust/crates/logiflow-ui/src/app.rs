//! 설정 창 — C# `MainForm`(사이드바 + 전환 관리·설정·진단 3화면)과 대화상자(진행 모달·라이선스).
//! 상태의 주인은 트레이 — 여기서는 보기와 편집 요청만(IPC).
//!
//! rs.3: 처음 쓰는 사람 점검 20건을 반영해 C# 1:1 에서 벗어났다(회장님 지시 "일단 다 고쳐서 포팅해줘봐").
//! 글은 고정 높이 상자 대신 남는 폭에서 줄바꿈하고 줄 높이가 글에 맞춰 늘어난다(글자 크기 3단 모두 잘림 없음).
//! 같은 일은 한 곳에만 두고(가장자리 자동 전환 → 전환 관리 · 기기 다시 찾기 → 전환 관리 · 기록 → 진단),
//! 단추 글은 동작만, 파랑은 누를 수 있는 것에만 쓴다. 바뀐 곳 전체는 `docs/RELEASE_NOTES_v0.16.0-rs.3.md`.

use crate::canvas::{Canvas, Screen};
use crate::paint::{c, nav_button, thumb_color, Icon, Pc, Variant};
use crate::platform::sys;
use egui::{Color32, CornerRadius, Event, Id, MouseWheelUnit, Pos2, Rect, Sense, Ui, Vec2, ViewportBuilder, ViewportCommand, ViewportId};
use logiflow_core::config::{AppConfig, EdgeAttachment, ThemeMode};
use logiflow_core::log::read_lines;
use logiflow_core::probe_result::{self, ProbeVerdict};
use logiflow_core::scan::{self, looks_like_keyboard, ScanResult};
use logiflow_core::theme::{Palette, Rgb};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sys::dialog::*;
use sys::util::message_box;
use sys::wnd::Hwnd;
use sys::{clipboard, firewall, ipc, legacy, monitors, notify, paths, prelogon, probe, prompt, theme_sys, tray};

pub const CONTENT_MAX: f32 = 560.0;
const SIDEBAR_W: f32 = 182.0;
const BAR_W: f32 = 10.0;

/// 줄 제목 · 설명 · 카드 안 좌우 여백 · 줄 위아래 여백 · 단추 사이.
const LAB: f32 = 9.5;
const DESC: f32 = 8.5;
const PX: f32 = 16.0;
const ROW_V: f32 = 14.0;
const GAP: f32 = 8.0;
/// 묶음(머리 + 카드) 사이 · 묶음 머리와 카드 사이(0.16.2: 세 화면 같은 값, 8 의 배수).
const GROUP_GAP: f32 = 24.0;
const HEAD_GAP: f32 = 8.0;
/// 되돌리기 기록 상한.
const UNDO_MAX: usize = 20;

/// 트레이에서 받은 상태(IPC 창 → 여기).
#[derive(Default)]
pub struct Shared {
    pub config: AppConfig,
    pub devices: Vec<ScanResult>,
    pub scanning: bool,
    pub clip_running: bool,
    pub switch_result: Option<bool>,
    pub activate: bool,
    /// 암호문구 보관 문제(Mac 키체인에 저장·읽기 실패) — 암호문구 줄에 주의 글로. Windows 는 늘 None.
    pub secret_note: Option<String>,
}

struct Modal {
    title: String,
    step: Arc<Mutex<String>>,
    finished: Arc<Mutex<Option<(String, bool)>>>,
    /// 끝난 뒤 설정에 반영할 「로그인 전에도 실행」 값.
    pre_logon_after: Arc<Mutex<Option<bool>>>,
    /// 기존 LogiFlow 정리를 했으면 닫을 때 남은 것을 다시 보고 트레이 감시를 다시 구성한다.
    after_cleanup: bool,
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
    /// 기기 목록 「문의용 번호 보기」 — 기기마다 문의할 때 알려 줄 번호(제품 번호 등)와 전환 안 되는 장치까지.
    detail: bool,
    /// 설정 「다른 주소 보기」 — 이 컴퓨터의 첫 주소 말고 다른 주소(기본 접힘).
    other_ips: bool,
    prelogon_installed: Arc<Mutex<bool>>,
    /// 방화벽 규칙 · 로그인 화면 테스트 작업이 있는지(None = 확인 중). 백그라운드로 조회.
    firewall_on: Arc<Mutex<Option<bool>>>,
    probe_installed: Arc<Mutex<Option<bool>>>,
    /// 기존 LogiFlow(C# 판)의 남은 것(None = 확인 중).
    legacy: Arc<Mutex<Option<legacy::Leftovers>>>,
    /// 이 PC 주소(호스트 이름 조회라 느릴 수 있어 백그라운드 1회).
    local_ips: Arc<Mutex<Option<Vec<clipboard::LocalAddr>>>>,
    canvas: Canvas,
    /// 채널 배치 되돌리기(바꾸기 전 배치를 쌓는다).
    layout_undo: Vec<Vec<EdgeAttachment>>,
    /// 「복사」를 누른 주소 번호와 시각(2초 동안 「복사함」).
    copied: Option<(usize, Instant)>,
    /// 「보내 보기」 마지막 결과(0 닿지 않음 · 1 주소에만 닿음 · 2 시험 글 보냄, 시:분, 확인할 때의 주소·포트·암호문구
    /// — 바뀌면 결과를 보이지 않음). 받는 쪽 암호문구가 맞는지는 보내는 쪽이 알 수 없어 「연결됨」이라 하지 않는다.
    last_link: Option<(u8, String, String)>,
    /// 「로그인 화면 테스트 열기」 — 진단 화면을 열고 테스트 묶음까지 내려간다.
    jump_test: bool,
    test_y: f32,
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

/// 화면 캡처 한 장 = (화면, 다크, 글자 단계, 아래로 스크롤, 모양: 0 기본 · 1 클립보드 공유 끔 · 2 기기 문의용 번호 보기 ·
/// 3 가운데쯤 스크롤 · 4 이전 판 남음 · 5 기기 없음 · 6 가운데쯤 + 다른 주소 펼침).
pub type SnapStep = (usize, bool, i32, bool, u8);

pub struct Snapshot {
    pub dir: std::path::PathBuf,
    pub steps: Vec<SnapStep>,
    pub idx: usize,
    pub frames: u32,
    pub requested: bool,
    pub done: Arc<std::sync::atomic::AtomicBool>,
    /// 「기기 없음」 단계 동안 치워 둔 기기 목록.
    pub stash: Vec<ScanResult>,
}

/// 이 PC 주소 다시 조회(설정 화면을 열 때마다 — 와이파이·VPN 으로 바뀌었을 수 있다). 조회 중엔 옛 값 유지.
fn refresh_ips(slot: &Arc<Mutex<Option<Vec<clipboard::LocalAddr>>>>) {
    let slot = slot.clone();
    std::thread::spawn(move || {
        let v = clipboard::local_addresses();
        *slot.lock().unwrap() = Some(v);
    });
}

/// 기존 LogiFlow 남은 것 다시 보기(서비스 관리자·레지스트리 조회라 백그라운드).
fn refresh_legacy(slot: &Arc<Mutex<Option<legacy::Leftovers>>>) {
    let slot = slot.clone();
    std::thread::spawn(move || {
        let v = legacy::leftovers();
        *slot.lock().unwrap() = Some(v);
    });
}

/// 백그라운드 상태 조회. 조회하는 동안은 옛 값을 그대로 보여 준다(처음에만 「확인 중…」).
fn spawn_check(slot: &Arc<Mutex<Option<bool>>>, f: impl FnOnce() -> bool + Send + 'static) {
    let slot = slot.clone();
    std::thread::spawn(move || {
        let v = f();
        *slot.lock().unwrap() = Some(v);
    });
}

impl SettingsApp {
    pub fn new(shared: Arc<Mutex<Shared>>, tray: Option<Hwnd>, version: String) -> Self {
        let installed = Arc::new(Mutex::new(false));
        {
            // 로그인 전 서비스 존재를 백그라운드로 1회 조회(설정 탭 지연 방지).
            let installed = installed.clone();
            std::thread::spawn(move || {
                let v = prelogon::is_installed();
                *installed.lock().unwrap() = v;
            });
        }
        let firewall_on = Arc::new(Mutex::new(None));
        let port = shared.lock().unwrap().config.clip_port;
        spawn_check(&firewall_on, move || firewall::allowed_for_port(port));
        let probe_installed = Arc::new(Mutex::new(None));
        spawn_check(&probe_installed, probe::is_installed);
        let local_ips = Arc::new(Mutex::new(None));
        refresh_ips(&local_ips);
        let legacy_state = Arc::new(Mutex::new(None));
        refresh_legacy(&legacy_state);
        SettingsApp {
            shared,
            tray,
            hwnd: None,
            page: 0,
            offset: 0.0,
            content_h: 0.0,
            thumb_drag: false,
            detail: false,
            other_ips: false,
            prelogon_installed: installed,
            firewall_on,
            probe_installed,
            local_ips,
            legacy: legacy_state,
            canvas: Canvas::default(),
            layout_undo: Vec::new(),
            copied: None,
            last_link: None,
            jump_test: false,
            test_y: 0.0,
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

    /// 배치를 바꾸기 전 지금 배치를 되돌리기 기록에 쌓고 새 배치를 적용.
    fn set_layout(&mut self, cfg: &AppConfig, att: Vec<EdgeAttachment>) {
        if att == cfg.attachments {
            return;
        }
        self.layout_undo.push(cfg.attachments.clone());
        if self.layout_undo.len() > UNDO_MAX {
            self.layout_undo.remove(0);
        }
        self.apply(|c| c.attachments = att);
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

    fn ask(&self, text: &str, title: &str) -> bool {
        self.msg(text, title, MB_YESNO | MB_ICONQUESTION) == IDYES
    }

    /// egui 좌표(point) → 화면 좌표(메뉴 위치). Windows 는 물리 픽셀, Mac 은 point(OS 별 dialog::to_screen).
    fn screen_point(&self, ctx: &egui::Context, p: Pos2) -> (i32, i32) {
        to_screen(self.hwnd, p.x, p.y, ctx.pixels_per_point())
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

    /// 값 바꾸기 입력 상자. 확인이면 Some(정규화한 값).
    fn prompt(&self, title: &str, label: &str, cur: &str, p: &Palette) -> Option<String> {
        let h = self.hwnd?;
        prompt::show(h, title, label, cur, p.content, p.ink).map(|v| logiflow_core::clip::norm(&v))
    }

    /// 가린 입력 상자(암호문구) — ● 로 가리고 눈 아이콘으로 보기/숨기기.
    fn prompt_secret(&self, title: &str, label: &str, cur: &str, p: &Palette) -> Option<String> {
        let h = self.hwnd?;
        prompt::show_secret(h, title, label, cur, p.content, p.ink).map(|v| logiflow_core::clip::norm(&v))
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
/// 스캔 결과의 연결 방식을 쉬운 말로(파일 형식은 그대로 두고 화면에서만 바꾼다).
/// (연결 방식, 리시버 안 번호). 번호는 채널 번호와 헷갈리므로 「자세히 보기」에서만 보인다.
fn connection_text(raw: &str) -> (String, Option<String>) {
    if raw.starts_with("직접") {
        return ("USB·블루투스 직접 연결".into(), None);
    }
    if let Some(rest) = raw.strip_prefix("리시버 뒤(index ") {
        if let Some(n) = rest.strip_suffix(')') {
            return ("리시버로 연결".into(), Some(n.to_string()));
        }
    }
    (raw.to_string(), None)
}
/// 「이전 판 지우기」가 하는 일 — 전환 관리 알림 · 진단 카드 · 확인 상자에 같은 글(첫 사용 점검 0.16.2: 두 곳 설명이 달랐다).
const LEGACY_WHAT: &str = "이전 판 앱·로그인 전 실행 서비스·예약 작업·자동 시작 항목·남은 파일을 지웁니다. 지금 쓰는 LogiFlow 와 설정은 그대로 남습니다. 관리자 승인이 한 번 필요합니다.";

/// 확인 상자 끝줄 — 이 PC 로 되돌아오는 길(전환하면 이 PC 에서 마우스·키보드를 잃으므로).
fn come_back(this_channel: i32) -> String {
    if this_channel >= 0 {
        format!("이 PC 로 되돌리려면 기기의 Easy-Switch 버튼에서 {}번을 누르세요.", this_channel + 1)
    } else {
        "이 PC 로 되돌리려면 기기의 Easy-Switch 버튼에서 이 PC 의 채널 번호를 누르세요.".into()
    }
}

/// 「모두 전환」 끝줄 — 기기마다 지금 채널이 다르면 기기별 번호를 적는다.
fn come_back_all(sw: &[ScanResult], current: i32) -> String {
    let chans: Vec<i32> = sw.iter().map(|d| d.current_host).collect();
    if chans.iter().all(|c| *c == current) {
        return come_back(current);
    }
    let each: Vec<String> = sw
        .iter()
        .filter(|d| d.current_host >= 0)
        .map(|d| format!("{} {}번", d.device_name, d.current_host + 1))
        .collect();
    format!("이 PC 로 되돌리려면 기기마다 Easy-Switch 버튼에서 지금 번호를 누르세요({}).", each.join(" · "))
}

fn log_dir() -> std::path::PathBuf {
    sys::util::exe_dir().join("logs")
}
fn has_logs() -> bool {
    std::fs::read_dir(log_dir()).map(|mut d| d.next().is_some()).unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
const LICENSE_BODY: &str = "LogiFlow\r\n로지텍 Easy-Switch 모니터 가장자리 자동 전환\r\n\r\nMIT License\r\n\r\nCopyright (c) 2026 happy4ed\r\n\r\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the \"Software\"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:\r\n\r\nThe above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.\r\n\r\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.\r\n\r\n─────────────────────────────\r\n구성 / 사용 기술\r\n• Rust · egui / eframe · wgpu (MIT / Apache-2.0)\r\n• windows-rs (Microsoft, MIT / Apache-2.0) — Windows HID / SetupAPI — Logitech HID++ 2.0 (feature 0x1814 Change Host)\r\n• windows-service · arboard · png (MIT / Apache-2.0)\r\n• RustCrypto aes · cbc · hmac · sha1 · sha2 · pbkdf2 (MIT / Apache-2.0)\r\n• chrono · unicode-normalization · getrandom · subtle (MIT / Apache-2.0 · BSD-3-Clause)\r\n• 설치 패키지: Inno Setup\r\n";
#[cfg(target_os = "macos")]
const LICENSE_BODY: &str = "LogiFlow\r\n로지텍 Easy-Switch 모니터 가장자리 자동 전환\r\n\r\nMIT License\r\n\r\nCopyright (c) 2026 happy4ed\r\n\r\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the \"Software\"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:\r\n\r\nThe above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.\r\n\r\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.\r\n\r\n─────────────────────────────\r\n구성 / 사용 기술\r\n• Rust · egui / eframe · wgpu (MIT / Apache-2.0)\r\n• objc2 · objc2-app-kit (MIT) — macOS IOKit HID — Logitech HID++ 2.0 (feature 0x1814 Change Host)\r\n• arboard · png (MIT / Apache-2.0)\r\n• RustCrypto aes · cbc · hmac · sha1 · sha2 · pbkdf2 (MIT / Apache-2.0)\r\n• chrono · unicode-normalization · getrandom · subtle · skrifa (MIT / Apache-2.0 · BSD-3-Clause)\r\n";

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
            if ok {
                self.msg("전환 명령을 보냈습니다.", "채널 전환", MB_OK | MB_ICONINFORMATION);
            } else {
                self.msg(
                    "전환하지 못했습니다. 기기가 켜져 있는지 확인하세요.\r\n계속 안 되면 「진단」에서 기록을 남겨 확인하세요.",
                    "채널 전환",
                    MB_OK | MB_ICONWARNING,
                );
            }
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
        // 0.16.1: 사이드바 맨 위 앱 이름(아이콘 + LogiFlow)은 뺐다 — 창 제목 막대와 두 번 보였다(첫 사용 점검).
        let nav = [("전환 관리", Icon::Swap), ("설정", Icon::Gear), ("진단", Icon::Diag)];
        let nav_h = (sb.line_h(9.5, false) + 14.0).max(36.0);
        for (i, (t, ic)) in nav.iter().enumerate() {
            let y = 18.0 + i as f32 * (nav_h + 4.0);
            if nav_button(&sb, &format!("nav{}", i), 12.0, y, 158.0, nav_h, t, *ic, self.page == i) && self.page != i {
                self.page = i;
                self.offset = 0.0; // C#: 화면 바꾸면 맨 위로
                refresh_legacy(&self.legacy);
                if i == 1 {
                    refresh_ips(&self.local_ips);
                    let port = cfg.clip_port;
                    spawn_check(&self.firewall_on, move || firewall::allowed_for_port(port));
                }
            }
        }
        let h = full.height();
        // 사이드바 아래 버전 — 사이드바 폭 안에서 줄바꿈. (C# 의 「LogiFlow 실행 중」 표시는 창이 열려 있으면
        // 늘 참이라 전하는 정보가 없고 단추처럼 보여 rs.3 에서 뺐다 — 첫 사용 점검.)
        let ver = format!("v{}", self.version);
        let ver_h = sb.wrap_h(&ver, 8.0, false, SIDEBAR_W - 28.0);
        let ver_y = h - 14.0 - ver_h;
        sb.wrap(20.0, ver_y, SIDEBAR_W - 28.0, &ver, 8.0, false, pal.muted);

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
        let drawn_page = self.page;
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
                2 => self.diag_page(&pc, &cfg, page_w),
                _ => self.switch_page(&pc, &ctx, page_w, &cfg, &devices, scanning),
            }
        };
        self.content_h = content_h;
        // 진단 화면을 실제로 그려 테스트 묶음 위치(test_y)를 안 다음 프레임에 내려간다.
        if self.jump_test && drawn_page == 2 && self.page == 2 {
            self.jump_test = false;
            self.offset = (self.test_y - 12.0).max(0.0);
            ctx.request_repaint();
        }

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

// ================= 줄 배치 도우미 =================

/// 화면 제목(이름만) + 설명(줄바꿈). 다음 y.
fn page_header(pc: &Pc<'_>, w: f32, title: &str, sub: &str) -> f32 {
    let mut y = 20.0;
    pc.text(24.0, y, title, 14.0, true, pc.pal.ink);
    y += pc.line_h(14.0, true) + 4.0;
    y += pc.wrap(24.0, y, w, sub, 9.0, false, pc.pal.muted);
    y + GROUP_GAP
}

/// 카드 위 작은 묶음 이름. 다음 y.
/// 0.16.2 묶음 규칙(세 화면 같음): 머리에는 이름만(개수·링크·버튼은 카드 안 본문으로), 내용은 모두 카드 안.
fn section(pc: &Pc<'_>, y: f32, title: &str) -> f32 {
    pc.text(24.0, y, title, 8.0, true, pc.pal.muted);
    y + pc.line_h(8.0, true) + HEAD_GAP
}

/// 카드 안 한 줄의 글 — 제목, 값(있으면), 설명.
struct Line<'s> {
    lab: &'s str,
    /// (글, 색, 굵게) — 값은 굵게, 상태는 보통 굵기.
    value: Option<(String, Rgb, bool)>,
    desc: String,
}

impl<'s> Line<'s> {
    fn new(lab: &'s str, desc: impl Into<String>) -> Self {
        Line {
            lab,
            value: None,
            desc: desc.into(),
        }
    }
    fn value(mut self, v: impl Into<String>, color: Rgb) -> Self {
        self.value = Some((v.into(), color, true));
        self
    }
    fn status(mut self, v: impl Into<String>, color: Rgb) -> Self {
        self.value = Some((v.into(), color, false));
        self
    }
}

/// 줄 글을 그리고 줄 높이를 돌려준다. right_w = 오른쪽 조작 영역 폭 — 글은 남는 폭에서 줄바꿈한다.
fn row_text(pc: &Pc<'_>, cx: f32, y: f32, w: f32, right_w: f32, l: &Line<'_>) -> f32 {
    let x = cx + PX;
    let gap = if right_w > 0.0 { 16.0 } else { 0.0 };
    let tw = (w - 2.0 * PX - right_w - gap).max(120.0);
    let mut yy = y + ROW_V;
    // 제목 없는 줄(설명 + 단추만)은 설명부터.
    let has_lab = !l.lab.is_empty();
    if has_lab {
        pc.text(x, yy, l.lab, LAB, true, pc.pal.ink);
        yy += pc.line_h(LAB, true);
    }
    if let Some((v, col, bold)) = &l.value {
        yy += 2.0;
        yy += pc.wrap(x, yy, tw, v, 9.0, *bold, *col);
    }
    if !l.desc.is_empty() {
        if has_lab || l.value.is_some() {
            yy += 3.0;
        }
        yy += pc.wrap(x, yy, tw, &l.desc, DESC, false, pc.pal.muted);
    }
    (yy + ROW_V - y).max(56.0)
}

/// 오른쪽에 토글이 있는 줄. (눌림, 높이).
fn toggle_row(pc: &Pc<'_>, key: &str, cx: f32, y: f32, w: f32, l: Line<'_>, on: bool) -> (bool, f32) {
    let h = row_text(pc, cx, y, w, 40.0, &l);
    let clicked = pc.toggle(key, cx + w - PX - 40.0, y + (h - 22.0) / 2.0, on);
    (clicked, h)
}

type Btn<'a> = (&'a str, Option<Icon>, Variant);

fn buttons_w(pc: &Pc<'_>, btns: &[Btn<'_>]) -> f32 {
    btns.iter().map(|(t, ic, _)| pc.btn_w(t, *ic)).sum::<f32>() + GAP * btns.len().saturating_sub(1) as f32
}

/// 오른쪽에 단추가 있는 줄(같은 성격의 단추만 한 줄에). (눌린 단추 번호, 높이).
fn buttons_row(pc: &Pc<'_>, key: &str, cx: f32, y: f32, w: f32, l: Line<'_>, btns: &[Btn<'_>]) -> (Option<usize>, f32) {
    let total = buttons_w(pc, btns);
    let h = row_text(pc, cx, y, w, total, &l);
    let bh = pc.btn_h();
    let mut bx = cx + w - PX - total;
    let by = y + (h - bh) / 2.0;
    let mut hit = None;
    for (i, (t, ic, v)) in btns.iter().enumerate() {
        let bw = pc.btn_w(t, *ic);
        if pc.button(&format!("{}{}", key, i), bx, by, bw, bh, t, *ic, *v, true) {
            hit = Some(i);
        }
        bx += bw + GAP;
    }
    (hit, h)
}

/// 카드 안 줄 사이 구분선.
fn divider(pc: &Pc<'_>, cx: f32, y: f32, w: f32) {
    pc.hline(cx + PX, y, w - 2.0 * PX);
}

impl SettingsApp {
    // ================= 전환 관리 =================
    fn switch_page(&mut self, pc: &Pc<'_>, ctx: &egui::Context, vw: f32, cfg: &AppConfig, devices: &[ScanResult], scanning: bool) -> f32 {
        let p = pc.pal;
        let w = cw(vw);
        let x0 = 24.0;
        let mut y = page_header(
            pc,
            w,
            "전환 관리",
            "채널은 마우스·키보드의 Easy-Switch 버튼 번호(1~3)이고, 버튼마다 PC 가 하나씩 짝지어져 있습니다.",
        );

        // 이전 판이 남아 트레이가 가장자리 감시를 멈춘 상태 — 화면 맨 위에서 바로 정리(캔버스·스위치만 보고
        // 「되는데 안 넘어간다」고 헤매지 않게).
        let legacy_now = self.legacy.lock().unwrap().clone();
        // 붙인 상자가 지금은 넘기지 않는 때(이전 판 때문에 멈춤 · 「사용」 꺼짐) — 캔버스에 「멈춤」.
        let paused = legacy_now.as_ref().map(|l| l.conflicts()).unwrap_or(false) || !cfg.edge_switch_enabled;
        if let Some(l) = legacy_now.filter(|l| l.conflicts()) {
            let slot = pc.card_begin();
            let top = y;
            let (hit, h) = buttons_row(
                pc,
                "legacy1",
                x0,
                y,
                w,
                Line::new(
                    "가장자리 자동 전환이 멈춰 있습니다",
                    format!(
                        "이전 판이 남아 있어 두 앱이 함께 넘기지 않도록 멈춰 두었습니다. 남은 것: {}. {}",
                        l.describe(),
                        LEGACY_WHAT
                    ),
                )
                .status("지우면 바로 다시 켜집니다", p.warn),
                &[("이전 판 지우기", None, Variant::Default)],
            );
            if hit.is_some() {
                self.start_legacy_cleanup(cfg, false);
            }
            y += h;
            pc.card_end(slot, x0, top, w, y - top);
            y += 16.0;
        }

        // 캔버스(이 PC 모니터 + 다른 채널 상자).
        let max_host = max_host_count(devices);
        let current = current_channel(devices);
        let channels: Vec<i32> = (0..max_host).filter(|h| *h != current).collect();
        // 기기를 못 찾아 채널 상자가 없을 때 — 캔버스 안에 이유와 할 일(0.16.1 첫 사용 점검: 모니터만 보여 막힘).
        // 「다시 찾기」는 이 안내 바로 옆에 두고, 이때는 아래 「기기」 묶음의 「다시 찾기」를 숨긴다(진입점은 늘 하나 —
        // 0.16.2 첫 사용 점검: 안내가 스크롤 맨 아래 단추를 가리켰다).
        let bh = pc.btn_h();
        let rescan = if scanning { "찾는 중…" } else { "다시 찾기" };
        let rw = pc.btn_w(rescan, Some(Icon::Refresh));
        let empty_msg = if scanning {
            "기기를 찾는 중입니다…"
        } else {
            "로지텍 마우스·키보드를 찾지 못해 붙일 채널 상자가 없습니다. 기기를 켜거나 리시버를 꽂은 뒤 「다시 찾기」를 누르세요."
        };
        let ecw = (w - 64.0).min(472.0);
        let emw = ecw - 32.0 - rw - 16.0;
        let emh = pc.wrap_h(empty_msg, DESC, false, emw);
        let ech = emh.max(bh) + 24.0;
        // 안내 상자가 가운데 모니터와 겹치지 않게 캔버스를 늘린다(모니터 = 캔버스 높이의 약 1/4, 가운데 —
        // 안내 위 = 캔버스 높이 − 상자 − 16 ≥ 모니터 아래 + 8 이 되는 높이, 「크게」에서 안내가 길어질 때).
        let canvas_h: f32 = if channels.is_empty() {
            300f32.max(((ech + 24.0) * 8.0 / 3.0).ceil())
        } else {
            300.0
        };
        let screens: Vec<Screen> = monitors::all()
            .into_iter()
            .map(|m| Screen {
                name: m.name,
                bounds: m.bounds,
                primary: m.primary,
            })
            .collect();
        self.canvas.sync(&channels, current, &cfg.attachments, screens);
        let label = |ch: i32| cfg.channel_label(ch);
        // 모니터 상자 아래 「이 PC · 채널 1」 — 모니터 번호와 채널 번호를 헷갈리지 않게 이 PC 의 실제 채널을 붙인다(0.16.2).
        let this_pc: Vec<String> = if current >= 0 {
            vec!["이 PC".into(), format!("채널 {}", current + 1)]
        } else {
            vec!["이 PC".into()]
        };
        let this_pc: Vec<String> = this_pc.iter().map(|t| crate::os_text(t).into_owned()).collect();
        if let Some(att) = self.canvas.show(
            pc.ui,
            pc.origin + Vec2::new(x0, y),
            Vec2::new(w, canvas_h),
            p,
            pc.off,
            pc.interactive,
            &label,
            &this_pc,
            paused,
        ) {
            self.set_layout(cfg, att);
        }
        if channels.is_empty() {
            let cx_ = x0 + (w - ecw) / 2.0;
            let cy_ = y + canvas_h - ech - 16.0;
            pc.card(cx_, cy_, ecw, ech);
            pc.wrap(cx_ + 16.0, cy_ + (ech - emh) / 2.0, emw, empty_msg, DESC, false, p.muted);
            if pc.button(
                "rescan0",
                cx_ + ecw - 16.0 - rw,
                cy_ + (ech - bh) / 2.0,
                rw,
                bh,
                rescan,
                Some(Icon::Refresh),
                Variant::Default,
                !scanning,
            ) {
                self.rescan();
            }
        }
        y += canvas_h + 8.0;
        // 캔버스 바로 아래 한 줄: 왼쪽 붙이기·떼기 방법(상단 설명에 묻혀 있던 것을 캔버스 옆으로 — 0.16.1 첫 사용 점검),
        // 오른쪽 「되돌리기」(배치를 바꾼 적이 있을 때 · Ctrl+Z / ⌘Z 도 같음). rs.4: 「채널 모두 떼기」는 뺐다 — 결과가
        // 「사용」 스위치를 끈 것과 같아(밀어도 안 넘어감) 잠깐 끄려던 사람이 배치를 지우게 됐다(8차 첫 사용 점검).
        let undo_key = pc.interactive && ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z));
        let uw = if self.layout_undo.is_empty() { 0.0 } else { pc.btn_w("되돌리기", None) };
        let hint = if channels.is_empty() {
            ""
        } else {
            "상자를 끌어 모니터 가장자리에 붙이고, 모니터에서 멀리 끌어 놓으면 뗍니다."
        };
        let hint_w = (w - uw - if uw > 0.0 { 16.0 } else { 0.0 }).max(120.0);
        let hint_h = if hint.is_empty() { 0.0 } else { pc.wrap_h(hint, DESC, false, hint_w) };
        let row_h = hint_h.max(if uw > 0.0 { bh } else { 0.0 });
        pc.wrap(x0, y + (row_h - hint_h) / 2.0, hint_w, hint, DESC, false, p.muted);
        if uw > 0.0 {
            let clicked = pc.button("undo", x0 + w - uw, y + (row_h - bh) / 2.0, uw, bh, "되돌리기", None, Variant::Default, true);
            if clicked || undo_key {
                if let Some(prev) = self.layout_undo.pop() {
                    self.apply(|c| c.attachments = prev);
                }
            }
        }
        y += row_h + GROUP_GAP;

        // 가장자리 자동 전환(이 화면에만 둔다 — 배치와 함께 쓰는 설정).
        y = section(pc, y, "가장자리 자동 전환");
        let slot = pc.card_begin();
        let top = y;
        let (hit, h) = toggle_row(
            pc,
            "edge",
            x0,
            y,
            w,
            Line::new(
                "사용",
                "붙여 둔 가장자리로 커서를 밀면 마우스·키보드가 모두 그 채널로 넘어갑니다. 끄면 아래 「모두 전환」이나 트레이 메뉴로 직접 넘길 때만 넘어갑니다.",
            ),
            cfg.edge_switch_enabled,
        );
        if hit {
            self.apply(|c| c.edge_switch_enabled = !c.edge_switch_enabled);
        }
        y += h;
        divider(pc, x0, y, w);
        let dwell_lab = dwell_text(cfg.dwell_ms);
        let (hit, h) = buttons_row(
            pc,
            "dwell",
            x0,
            y,
            w,
            Line::new(
                "넘어가기 전 대기",
                "가장자리에서 이만큼 밀고 있어야 넘어갑니다. 짧으면 빨리 넘어가고, 길면 실수로 넘어가는 일이 줄어듭니다. 기본 0.33초.",
            ),
            &[(&dwell_lab, Some(Icon::ChevronDown), Variant::Default)],
        );
        if hit.is_some() {
            let bw = pc.btn_w(&dwell_lab, Some(Icon::ChevronDown));
            let anchor = pc.r(x0 + w - PX - bw, y + (h + bh) / 2.0, bw, 0.0).left_top();
            let opts = [200, 330, 500, 800, 1000];
            let items: Vec<notify::MenuItem> = opts
                .iter()
                .enumerate()
                .map(|(i, ms)| notify::MenuItem::Item {
                    id: 100 + i as u32,
                    text: if *ms == logiflow_core::edge::DEFAULT_DWELL_MS {
                        format!("{} (기본)", dwell_text(*ms))
                    } else {
                        dwell_text(*ms)
                    },
                    enabled: true,
                })
                .collect();
            let r = self.menu(ctx, anchor, &items);
            if (100..105).contains(&r) {
                let v = opts[(r - 100) as usize];
                self.apply(|c| c.dwell_ms = v);
            }
        }
        y += h;
        divider(pc, x0, y, w);
        let (hit, h) = toggle_row(
            pc,
            "guard",
            x0,
            y,
            w,
            Line::new(
                "끌기 중 전환 막기",
                "마우스 버튼을 누른 채 창이나 파일을 끌고 있을 때는 가장자리에 닿아도 넘어가지 않습니다.",
            ),
            cfg.drag_guard,
        );
        if hit {
            self.apply(|c| c.drag_guard = !c.drag_guard);
        }
        y += h;
        pc.card_end(slot, x0, top, w, y - top);
        y += GROUP_GAP;

        // 채널 이름 — 다른 묶음처럼 머리(이름만) + 카드(설명 · 칩)(0.16.2 묶음 규칙).
        y = section(pc, y, "채널 이름");
        let slot = pc.card_begin();
        let top = y;
        let (tx, tw) = (x0 + PX, w - 2.0 * PX);
        y += ROW_V;
        y += pc.wrap(
            tx,
            y,
            tw,
            "채널마다 짝지어 둔 PC 를 알아보기 쉬운 이름을 붙입니다. 누르면 바꿀 수 있고, 비우면 원래 이름(예: 채널 3)으로 돌아갑니다.",
            DESC,
            false,
            p.muted,
        );
        y += 16.0;
        let count = max_host.max(1);
        let chip_w = ((tw - (count - 1) as f32 * GAP) / count as f32).floor().max(110.0);
        let chip_h = (pc.line_h(8.5, false) + 14.0).max(32.0);
        let mut ax = tx;
        for ch in 0..count {
            let chip = pc.r(ax, y, chip_w, chip_h);
            let resp = pc.click_area(&format!("chip{}", ch), chip);
            if resp.hovered() && pc.interactive {
                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            pc.card(ax, y, chip_w, chip_h);
            let lab = format!("채널 {}", ch + 1);
            let lab_w = pc.measure(&lab, 8.0, true);
            let ty = y + (chip_h - pc.line_h(8.5, false)) / 2.0;
            pc.text(ax + 10.0, ty, &lab, 8.0, true, p.ink);
            let pen_x = ax + chip_w - 24.0 - 6.0;
            let al_left = ax + 10.0 + lab_w + 10.0;
            let al_w = (pen_x - al_left - 6.0).max(10.0);
            // 이름을 비우면 원래 이름(「채널 3」)만 — 설명의 「원래 이름」과 같게(「이름 없음」은 쓰지 않음, 0.16.2).
            if cfg.aliases.iter().any(|(c, v)| *c == ch && !v.is_empty()) {
                pc.text_ellipsis(al_left, ty, al_w, &cfg.alias_for(ch), 8.5, false, p.ink);
            }
            pc.icon(Icon::Pencil, pen_x + 4.0, y + (chip_h - 16.0) / 2.0, 16.0, p.muted);
            if resp.clicked() && pc.interactive {
                self.edit_alias(ch, cfg, p);
            }
            ax += chip_w + GAP;
        }
        y += chip_h + ROW_V + 2.0;
        pc.card_end(slot, x0, top, w, y - top);
        y += GROUP_GAP;

        // 기기 — 목록은 이 화면 한 곳에만. 머리는 이름만, 찾은 개수 · 「다시 찾기」 · 「문의용 번호 보기」는 카드 안(0.16.2).
        let sw = switchable(devices);
        let shown: Vec<&ScanResult> = devices
            .iter()
            .filter(|r| self.detail || (r.change_host_supported && r.host_count > 0))
            .collect();
        y = section(pc, y, "기기");
        let slot = pc.card_begin();
        let top = y;
        // 채널 상자가 없으면 「다시 찾기」는 캔버스 안 안내 옆에만(위).
        let rw = if channels.is_empty() { 0.0 } else { rw };
        // 개수는 전환되는 기기만(리시버 본체가 보여도 고장처럼 읽히지 않게).
        let found = if !sw.is_empty() {
            format!("찾은 기기 {}개", sw.len())
        } else if scanning {
            "기기를 찾는 중".to_string()
        } else {
            "찾은 기기 없음".to_string()
        };
        // 못 찾았을 때 할 일은 캔버스 안 안내 한 곳에만(같은 문장을 두 번 쓰지 않음).
        let desc = if self.detail {
            "연결된 로지텍 마우스·키보드를 자동으로 찾습니다. 문의용 번호는 문제를 알릴 때 함께 보내는 번호라 평소에는 신경 쓰지 않아도 됩니다."
        } else {
            "연결된 로지텍 마우스·키보드를 자동으로 찾습니다."
        };
        let head = Line::new("", desc).value(found, if sw.is_empty() && !scanning { p.warn } else { p.ink });
        let h = row_text(pc, x0, y, w, rw, &head);
        if rw > 0.0
            && pc.button(
                "rescan",
                x0 + w - PX - rw,
                y + (h - bh) / 2.0,
                rw,
                bh,
                rescan,
                Some(Icon::Refresh),
                Variant::Default,
                !scanning,
            )
        {
            self.rescan();
        }
        // 「문의용 번호 보기」 ↔ 「문의용 번호 숨기기」 — 목록 표시만 바꾸므로 링크(다른 링크와 같은 글자 크기), 설명 바로 아래.
        if devices.is_empty() {
            y += h;
        } else {
            y += h - ROW_V + 2.0;
            let link = if self.detail {
                "문의용 번호 숨기기"
            } else {
                "문의용 번호 보기"
            };
            let link_w = pc.btn_w(link, None);
            if pc.button("detail", x0 + PX - 14.0, y, link_w, bh, link, None, Variant::Ghost, true) {
                self.detail = !self.detail;
            }
            y += bh + ROW_V - 4.0;
        }
        let one_label = "이 기기만 전환";
        let one_w = pc.btn_w(one_label, Some(Icon::ChevronDown)) + 10.0;
        let all_label = "모두 전환";
        // 같은 열의 두 단추(「이 기기만 전환」 · 「모두 전환」)는 같은 폭.
        let col_w = one_w.max(pc.btn_w(all_label, Some(Icon::ChevronDown)) + 10.0);
        let name_lh = pc.line_h(9.5, false);
        let (rx, rw_) = (x0 + PX, w - 2.0 * PX);
        for (i, d) in shown.iter().enumerate() {
            let ok = d.change_host_supported && d.host_count > 0;
            let is_receiver = d.device_name.to_ascii_lowercase().contains("receiver") || d.device_name.contains("리시버");
            let (conn, slot_no) = connection_text(&d.connection_type);
            let mut sub = if is_receiver && d.device_index == 0xFF {
                "리시버 본체".to_string()
            } else {
                conn
            };
            if ok {
                // 지금 붙어 있는 채널은 글로(알약 모양은 꺼진 단추처럼 보여서 쓰지 않음).
                if d.current_host >= 0 {
                    sub.push_str(&format!(" · 지금 채널 {}", d.current_host + 1));
                }
            } else {
                sub.push_str(" · 채널 전환 안 됨");
            }
            if self.detail {
                // 문의할 때 알려 주는 번호(제품 번호 · 리시버 안 번호 · 기기 안 전환 기능 번호). 뜻을 몰라도 되게 이름 없이 묶는다.
                let mut ids = vec![d.vid_pid.clone()];
                if let Some(n) = slot_no {
                    ids.push(n);
                }
                if d.change_host_supported {
                    ids.push(d.change_host_index.to_string());
                }
                sub.push_str(&format!("\n문의용\u{a0}번호\u{a0}{}", ids.join("\u{a0}·\u{a0}")));
            }
            divider(pc, x0, y, w);
            let bx = rx + rw_ - col_w;
            let text_x = rx + 28.0;
            let text_w = if ok { bx - 16.0 - text_x } else { rx + rw_ - text_x };
            let sub_h = pc.wrap_h(&sub, 8.0, false, text_w);
            let row_h = (name_lh + 2.0 + sub_h + 2.0 * ROW_V).max(bh + 2.0 * ROW_V).max(56.0);
            let ty = y + (row_h - (name_lh + 2.0 + sub_h)) / 2.0;
            let icon = if is_receiver || (!ok && !looks_like_keyboard(&d.device_name)) {
                Icon::Plug
            } else if looks_like_keyboard(&d.device_name) {
                Icon::Keyboard
            } else {
                Icon::Mouse
            };
            pc.icon(icon, rx, y + (row_h - 18.0) / 2.0, 18.0, p.muted);
            pc.text_ellipsis(text_x, ty, text_w, &d.device_name, 9.5, false, if ok { p.ink } else { p.muted });
            pc.wrap(text_x, ty + name_lh + 2.0, text_w, &sub, 8.0, false, p.muted);
            if ok
                && pc.button(
                    &format!("sw{}", i),
                    bx,
                    y + (row_h - bh) / 2.0,
                    col_w,
                    bh,
                    one_label,
                    Some(Icon::ChevronDown),
                    Variant::Default,
                    true,
                )
            {
                let anchor = pc.r(bx, y + (row_h - bh) / 2.0, col_w, bh).left_bottom();
                self.device_switch_menu(ctx, anchor, d, max_host, cfg);
            }
            y += row_h;
        }
        // 모두 전환(트레이 메뉴의 채널 항목과 같은 동작) — 설명 + 단추만 있는 줄.
        if !sw.is_empty() {
            divider(pc, x0, y, w);
            let line = Line::new("", "마우스·키보드를 한꺼번에 다른 채널의 PC 로 보냅니다.");
            let h = row_text(pc, x0, y, w, col_w, &line);
            let bx = x0 + w - PX - col_w;
            if pc.button(
                "swall",
                bx,
                y + (h - bh) / 2.0,
                col_w,
                bh,
                all_label,
                Some(Icon::ChevronDown),
                Variant::Default,
                true,
            ) {
                let anchor = pc.r(bx, y + (h - bh) / 2.0, col_w, bh).left_bottom();
                self.all_switch_menu(ctx, anchor, &sw, max_host, current, cfg);
            }
            y += h;
        }
        pc.card_end(slot, x0, top, w, y - top);
        y + 32.0
    }

    fn edit_alias(&mut self, ch: i32, cfg: &AppConfig, p: &Palette) {
        let cur = cfg.aliases.iter().find(|(c, _)| *c == ch).map(|(_, v)| v.clone()).unwrap_or_default();
        if let Some(v) = self.prompt("채널 이름", &format!("채널 {} 의 이름 (비우면 원래 이름 「채널 {}」)", ch + 1, ch + 1), &cur, p) {
            self.apply(|c| c.set_alias(ch, &v));
        }
    }

    /// 전환할 채널 메뉴(지금 채널 제외). 항목은 「채널 2 · 노트북으로 전환」처럼 동사까지. 고른 채널 또는 None.
    fn channel_menu(&self, ctx: &egui::Context, anchor: Pos2, hosts: i32, current: i32, cfg: &AppConfig, verb: &str) -> Option<i32> {
        let mut items = Vec::new();
        for h in 0..hosts {
            if h == current {
                continue;
            }
            items.push(notify::MenuItem::Item {
                id: 200 + h as u32,
                text: format!("{} {}", cfg.channel_label_to(h), verb),
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
            Some((r - 200) as i32)
        } else {
            None
        }
    }

    fn device_switch_menu(&mut self, ctx: &egui::Context, anchor: Pos2, d: &ScanResult, max_host: i32, cfg: &AppConfig) {
        let hosts = if d.host_count > 0 { d.host_count } else { max_host };
        let Some(target) = self.channel_menu(ctx, anchor, hosts, d.current_host, cfg, "전환") else {
            return;
        };
        if self.busy {
            return;
        }
        let q = format!(
            "{}\r\n→ {}\r\n\r\n이 기기를 전환할까요? 전환하면 이 PC 에서는 이 기기를 쓸 수 없습니다.\r\n{}",
            d.device_name,
            cfg.channel_label(target),
            come_back(d.current_host)
        );
        if self.msg(&q, "이 기기만 전환", MB_YESNO | MB_ICONWARNING) != IDYES {
            return;
        }
        self.busy = true;
        if let Some(t) = self.tray {
            ipc::send(t.get(), ipc::SWITCH_ONE, &format!("{}\t{}\t{}", d.path, d.device_index, target), 2000);
        }
    }

    fn all_switch_menu(&mut self, ctx: &egui::Context, anchor: Pos2, sw: &[ScanResult], max_host: i32, current: i32, cfg: &AppConfig) {
        let Some(target) = self.channel_menu(ctx, anchor, max_host, current, cfg, "모두 전환") else {
            return;
        };
        if self.busy {
            return;
        }
        let names: Vec<&str> = sw.iter().map(|d| d.device_name.as_str()).collect();
        let q = format!(
            "{}\r\n→ {}\r\n\r\n모두 전환할까요? 전환하면 이 PC 에서는 이 기기들을 쓸 수 없습니다.\r\n{}",
            names.join(" · "),
            cfg.channel_label(target),
            come_back_all(sw, current)
        );
        if self.msg(&q, "모두 전환", MB_YESNO | MB_ICONWARNING) != IDYES {
            return;
        }
        self.busy = true;
        if let Some(t) = self.tray {
            ipc::send(t.get(), ipc::SWITCH_ALL, &target.to_string(), 2000);
        }
    }

    // ================= 설정 =================
    fn settings_page(&mut self, pc: &Pc<'_>, ctx: &egui::Context, vw: f32, cfg: &AppConfig, clip_running: bool) -> f32 {
        let p = pc.pal;
        let w = cw(vw);
        let cx = 24.0;
        let bh = pc.btn_h();
        let mut y = page_header(pc, w, "설정", "앱 모양과 실행 방식, 클립보드 공유를 정합니다.");

        // 모양
        y = section(pc, y, "모양");
        let slot = pc.card_begin();
        let top = y;
        // 칸 폭은 가장 긴 글자 + 여백(크게 단계에서도 칸 가장자리에 붙지 않게).
        let seg_item = ["시스템", "라이트", "다크", "작게", "보통", "크게"]
            .iter()
            .map(|t| pc.measure(t, 10.0, false))
            .fold(0.0f32, f32::max);
        let seg_w = (3.0 * (seg_item + 36.0)).max(224.0).ceil();
        let seg_h = (pc.line_h(10.0, false) + 14.0).max(34.0);
        let h = row_text(pc, cx, y, w, seg_w, &Line::new("테마", "시스템 설정을 따르거나 라이트·다크로 고정합니다."));
        if let Some(i) = pc.segmented(
            "theme",
            cx + w - PX - seg_w,
            y + (h - seg_h) / 2.0,
            seg_w,
            seg_h,
            &["시스템", "라이트", "다크"],
            cfg.theme as usize,
        ) {
            self.apply(|c| c.theme = ThemeMode::from_i32(i as i32).unwrap_or_default());
        }
        y += h;
        divider(pc, cx, y, w);
        let h = row_text(
            pc,
            cx,
            y,
            w,
            seg_w,
            &Line::new("글자 크기", "보통이 기본입니다. 글자가 작아 보이면 크게로 바꾸세요."),
        );
        let font_idx = if cfg.font_offset >= 3 {
            2
        } else if cfg.font_offset <= -1 {
            0
        } else {
            1
        };
        if let Some(i) = pc.segmented(
            "font",
            cx + w - PX - seg_w,
            y + (h - seg_h) / 2.0,
            seg_w,
            seg_h,
            &["작게", "보통", "크게"],
            font_idx,
        ) {
            let off = match i {
                2 => 3,
                0 => -1,
                _ => 1,
            };
            self.apply(|c| c.font_offset = off);
        }
        y += h;
        pc.card_end(slot, cx, top, w, y - top);
        y += GROUP_GAP;

        // 실행
        y = section(pc, y, "실행");
        let slot = pc.card_begin();
        let top = y;
        let (hit, h) = toggle_row(
            pc,
            "autostart",
            cx,
            y,
            w,
            Line::new("Windows 시작 시 실행", "Windows 에 로그인하면 LogiFlow 가 알림 영역에서 자동으로 시작합니다."),
            cfg.auto_start,
        );
        if hit {
            self.apply(|c| c.auto_start = !c.auto_start);
        }
        y += h;
        // 로그인 전에도 실행 — Windows 만(서비스·로그인 화면). Mac 에는 없음.
        if CAPS.prelogon {
            divider(pc, cx, y, w);
            let installed = *self.prelogon_installed.lock().unwrap();
            let (hit, h) = toggle_row(
                pc,
                "prelogon",
                cx,
                y,
                w,
                Line::new(
                    "로그인 전에도 실행",
                    "PC 를 켠 직후의 로그인 화면과 잠금 화면에서도 가장자리로 밀면 넘어갑니다. 켜고 끌 때 관리자 승인이 필요하고, 「전환 관리」의 채널 배치를 그대로 씁니다.",
                ),
                installed,
            );
            if hit {
                self.toggle_prelogon(!installed, cfg);
            }
            y += h - ROW_V + 2.0;
            // 테스트로 가는 길(화면 이동이라 링크).
            let link = "진단 화면에서 테스트하기";
            let lw = pc.btn_w(link, None);
            if pc.button("gotest", cx + PX - 14.0, y, lw, bh, link, None, Variant::Ghost, true) {
                self.page = 2;
                self.jump_test = true;
            }
            y += bh + ROW_V - 4.0;
        }
        pc.card_end(slot, cx, top, w, y - top);
        y += GROUP_GAP;

        // 클립보드 공유 — 꺼져 있으면 켜기 줄만(아래 설정은 접힘).
        y = section(pc, y, "클립보드 공유");
        let slot = pc.card_begin();
        let top = y;
        let use_line = Line::new(
            "사용",
            "같은 네트워크에 있는 상대 컴퓨터 한 대와 복사한 글·이미지를 주고받습니다. 두 컴퓨터 모두 LogiFlow 를 켜 두어야 합니다.",
        );
        // 상태 한 줄 — 실제로 막히는 이유를 먼저(주소 → 암호문구 → 시작 실패 → 방화벽).
        let fw_now = *self.firewall_on.lock().unwrap();
        let use_line = if !cfg.clip_enabled {
            use_line
        } else if cfg.clip_peer_host.is_empty() {
            use_line.status("아직 못 씀 — 상대 컴퓨터 주소를 넣어 주세요", p.warn)
        } else if cfg.clip_secret.is_empty() {
            use_line.status("아직 못 씀 — 암호문구를 넣어 주세요", p.warn)
        } else if !clip_running {
            use_line.status("시작하지 못함 — 다른 프로그램이 포트를 쓰고 있을 수 있습니다", p.warn)
        } else if CAPS.firewall && fw_now == Some(false) {
            use_line.status("받기가 막혀 있을 수 있음 — 아래 「Windows 방화벽」에서 허용하세요", p.warn)
        } else {
            use_line.status("주고받을 준비됨", p.ink)
        };
        let (hit, h) = toggle_row(pc, "clip", cx, y, w, use_line, cfg.clip_enabled);
        if hit {
            self.apply(|c| c.clip_enabled = !c.clip_enabled);
        }
        y += h;
        if cfg.clip_enabled {
            // 상대 컴퓨터 주소 · 암호문구 — 값이 있으면 「바꾸기」 + ⋮(지우기).
            // 0.16.2: 주소 칸 이름은 두 OS 모두 「이 컴퓨터 주소」 · 「상대 컴퓨터 주소」(상대가 어느 OS 든 같은 이름 — 괄호 보충 없음).
            let secret_note = self.shared.lock().unwrap().secret_note.clone();
            for (key, lab, desc, value, prompt_label, is_secret) in [
                (
                    "peer",
                    "상대 컴퓨터 주소",
                    "상대 컴퓨터 설정 창에 보이는 「이 컴퓨터 주소」를 넣습니다.",
                    cfg.clip_peer_host.clone(),
                    "상대 컴퓨터의 「이 컴퓨터 주소」 (예: 192.168.0.20)",
                    false,
                ),
                (
                    "secret",
                    "암호문구",
                    "두 컴퓨터에 같은 값을 넣습니다. 다르면 주고받지 못합니다.",
                    cfg.clip_secret.clone(),
                    "두 컴퓨터에 같은 값",
                    true,
                ),
            ] {
                divider(pc, cx, y, w);
                let empty = value.is_empty();
                let line = Line::new(lab, desc);
                let note = if is_secret { secret_note.clone() } else { None };
                let line = match (empty, is_secret) {
                    _ if note.is_some() => line.status(note.unwrap_or_default(), p.warn),
                    (true, _) => line.value("아직 없음", p.warn),
                    (false, true) => line.value("입력함", p.ink),
                    (false, false) => line.value(value.clone(), p.ink),
                };
                let btns: Vec<Btn<'_>> = if empty {
                    vec![("입력", None, Variant::Default)]
                } else {
                    vec![("바꾸기", None, Variant::Default), ("", Some(Icon::More), Variant::Icon)]
                };
                let (hit, h) = buttons_row(pc, key, cx, y, w, line, &btns);
                match hit {
                    Some(0) => {
                        let v = if is_secret {
                            self.prompt_secret(lab, prompt_label, &value, p)
                        } else {
                            self.prompt(lab, prompt_label, &value, p)
                        };
                        if let Some(v) = v {
                            if is_secret {
                                self.apply(|c| c.clip_secret = v);
                            } else {
                                self.apply(|c| c.clip_peer_host = v);
                            }
                        }
                    }
                    Some(1) => {
                        let bw = pc.btn_w("", Some(Icon::More));
                        let anchor = pc.r(cx + w - PX - bw, y + (h + bh) / 2.0, bw, 0.0).left_top();
                        let items = [notify::MenuItem::Item {
                            id: 300,
                            text: "지우기".into(),
                            enabled: true,
                        }];
                        if self.menu(ctx, anchor, &items) == 300
                            && self.ask(
                                if is_secret {
                                    "암호문구를 지울까요?\r\n지우면 클립보드를 주고받지 않습니다."
                                } else {
                                    "상대 컴퓨터 주소를 지울까요?\r\n지우면 클립보드를 주고받지 않습니다."
                                },
                                lab,
                            )
                        {
                            if is_secret {
                                self.apply(|c| c.clip_secret = String::new());
                            } else {
                                self.apply(|c| c.clip_peer_host = String::new());
                            }
                        }
                    }
                    _ => {}
                }
                y += h;
                // 「상대 컴퓨터 주소」 바로 아래 「이 컴퓨터 주소」 — 두 주소를 붙여 둔다(첫 사용 점검 0.16.2).
                if key == "peer" {
                    y = self.local_addr_rows(pc, ctx, cx, y, w);
                }
            }
            // Windows 방화벽(Mac 에서는 숨김 — 들어오는 연결을 시스템이 처음 한 번 묻는다).
            if CAPS.firewall {
                divider(pc, cx, y, w);
                y += self.firewall_row(pc, cx, y, w, cfg, false);
            }
            // 연결 확인 — 마지막 결과를 상태로 남긴다.
            divider(pc, cx, y, w);
            let line = Line::new("연결 확인", "상대 컴퓨터로 시험 글을 한 번 보냅니다. 상대 컴퓨터에서 붙여 넣어 확인하세요.");
            let link_key = format!("{}|{}|{}", cfg.clip_peer_host, cfg.clip_port, cfg.clip_secret);
            let line = match &self.last_link {
                Some((2, t, k)) if *k == link_key => line.status(format!("{} 시험 글 보냄 — 상대 컴퓨터에서 붙여 넣어 확인하세요", t), p.ink),
                Some((1, t, k)) if *k == link_key => line.status(format!("{} 주소에는 닿음 — 암호문구를 넣으면 시험 글까지 보냅니다", t), p.ink),
                Some((0, t, k)) if *k == link_key => line.status(format!("{} 닿지 않음 — 주소와 방화벽을 확인하세요", t), p.warn),
                _ => line.status("아직 확인 안 함", p.muted),
            };
            let (hit, h) = buttons_row(pc, "ctest", cx, y, w, line, &[("보내 보기", None, Variant::Default)]);
            if hit.is_some() {
                let (ok, r) = if cfg.clip_peer_host.is_empty() {
                    (None, "상대 컴퓨터 주소를 먼저 넣어 주세요.".to_string())
                } else if cfg.clip_secret.is_empty() || !clip_running {
                    // 암호문구가 없거나 공유가 아직 시작 전이면 연결만 확인한다.
                    let (ok, m) = clipboard::test_connect(&cfg.clip_peer_host, cfg.clip_port);
                    (
                        Some(if ok { 1u8 } else { 0 }),
                        format!("{}\r\n\r\n암호문구를 넣고 공유가 시작되면 시험 글까지 보냅니다.", m),
                    )
                } else {
                    let (ok, m) = clipboard::send_test(true, cfg);
                    (Some(if ok { 2u8 } else { 0 }), m)
                };
                if let Some(ok) = ok {
                    let hm = sys::session::local_time_hms().get(..5).unwrap_or("").to_string();
                    self.last_link = Some((ok, hm, link_key.clone()));
                }
                self.msg(&r, "연결 확인", MB_OK | MB_ICONINFORMATION);
            }
            y += h;
        } else if CAPS.firewall && *self.firewall_on.lock().unwrap() == Some(true) {
            // 공유를 꺼도 허용 규칙은 남는다 — 보이게 두어 해제할 수 있게.
            divider(pc, cx, y, w);
            y += self.firewall_row(pc, cx, y, w, cfg, true);
        }
        pc.card_end(slot, cx, top, w, y - top);
        y += GROUP_GAP;

        // 정보
        y = section(pc, y, "정보");
        let slot = pc.card_begin();
        let top = y;
        let (hit, h) = buttons_row(
            pc,
            "lic",
            cx,
            y,
            w,
            Line::new("오픈소스 라이선스", "LogiFlow 와 함께 쓰인 구성 요소의 라이선스입니다."),
            &[("보기", None, Variant::Ghost)],
        );
        if hit.is_some() {
            self.license = true;
            self.license_offset = 0.0;
        }
        y += h;
        pc.card_end(slot, cx, top, w, y - top);
        y + 28.0
    }

    /// 「이전 판 지우기」(0.16.1 까지 「기존 LogiFlow 정리」 — 이름만 바뀜, 동작 같음) — 확인 상자 → 진행 창
    /// (사용자 쪽 정리 → UAC 한 번으로 서비스·예약 작업·남은 파일).
    /// then_enable_prelogon 이면 같은 UAC 로 「로그인 전에도 실행」까지 켠다(설치 단계가 남은 서비스를 먼저 지움).
    fn start_legacy_cleanup(&mut self, cfg: &AppConfig, then_enable_prelogon: bool) {
        let found = self.legacy.lock().unwrap().clone().unwrap_or_default();
        let what = if found.any() { found.describe() } else { "남은 흔적".to_string() };
        let content = format!(
            "남은 것: {}.\r\n\r\n{}{}",
            what,
            LEGACY_WHAT,
            if then_enable_prelogon {
                "\r\n지운 뒤 이 앱의 「로그인 전에도 실행」을 켭니다."
            } else {
                ""
            }
        );
        let pick = sys::util::task_dialog(
            self.owner(),
            "LogiFlow",
            "이전 판이 이 PC 에 남아 있습니다",
            &content,
            &[(1001, "이전 판 지우기")],
        );
        if pick != 1001 {
            return;
        }
        let mut modal = Modal::new(if then_enable_prelogon {
            "이전 판 지우고 로그인 전에도 실행 켜기"
        } else {
            "이전 판 지우기"
        });
        modal.after_cleanup = true;
        let (step, fin, after) = (modal.step.clone(), modal.finished.clone(), modal.pre_logon_after.clone());
        let cfg2 = cfg.clone();
        std::thread::spawn(move || {
            let log = logiflow_core::log::DiagLogger::new(&sys::util::exe_dir(), true);
            *step.lock().unwrap() = "이전 판 앱을 지우는 중…".into();
            let mut lines = legacy::cleanup_user(&log);
            if then_enable_prelogon {
                *step.lock().unwrap() = "채널 배치 저장 중…".into();
                tray::write_logon_edges(&cfg2);
                *step.lock().unwrap() = "관리자 승인을 요청하는 중…\r\n승인 창이 뜨면 「예」를 눌러 주세요.".into();
                // 설치 단계(--install-prelogon)가 같은 UAC 로 이전 판 서비스·작업·파일을 먼저 지운다.
                let ok = prelogon::request_enable(cfg2.my_host);
                *after.lock().unwrap() = Some(ok);
                let left = legacy::leftovers();
                lines.push(if left.conflicts() {
                    "이전 판 서비스가 아직 남아 있음".into()
                } else {
                    "이전 판 서비스·남은 파일 지움".into()
                });
                *fin.lock().unwrap() = Some((
                    if ok && !left.conflicts() {
                        "이전 판을 지우고 「로그인 전에도 실행」을 켰습니다.".to_string()
                    } else if ok {
                        "「로그인 전에도 실행」은 켰지만 이전 판의 일부가 남았습니다. 「진단」에서 「이전 판 지우기」를 다시 눌러 주세요.".to_string()
                    } else {
                        "켜지 못했습니다(취소했거나 관리자 승인이 막혔습니다).".to_string()
                    },
                    !ok,
                ));
            } else {
                *step.lock().unwrap() = "관리자 승인을 요청하는 중…\r\n승인 창이 뜨면 「예」를 눌러 주세요.".into();
                let (ok, more) = legacy::request_cleanup_machine_only();
                lines.extend(more);
                *fin.lock().unwrap() = Some((
                    if ok {
                        "이전 판을 지웠습니다. 이제 가장자리 자동 전환이 다시 켜집니다.".to_string()
                    } else {
                        "일부를 지우지 못했습니다(취소했거나 관리자 승인이 막혔습니다). 남은 앱은 Windows 설정 → 앱에서 「LogiFlow」를 지울 수 있습니다."
                            .to_string()
                    },
                    !ok,
                ));
            }
            log.log(&format!("[기존 LogiFlow 정리] 결과: {}", lines.join(" / ")));
        });
        self.modal = Some(modal);
    }

    /// 「이 컴퓨터 주소」 줄(첫 주소 + 접어 둔 다른 주소). 다음 y.
    fn local_addr_rows(&mut self, pc: &Pc<'_>, ctx: &egui::Context, cx: f32, mut y: f32, w: f32) -> f32 {
        let p = pc.pal;
        let bh = pc.btn_h();
        // 이 컴퓨터 주소 — 첫 주소만 「복사」와 함께 늘 보이고, 다른 주소는 접어 두었다가 「다른 주소 보기」로 편다
        // (보통 쓰지 않는 주소를 「복사」와 함께 늘 보이면 잘못 복사하기 쉽다 — 0.16.2).
        let ips_state = self.local_ips.lock().unwrap().clone();
        let ips = ips_state.clone().unwrap_or_default();
        if ips.is_empty() {
            divider(pc, cx, y, w);
            let line = Line::new("이 컴퓨터 주소", "이 주소를 상대 컴퓨터의 「상대 컴퓨터 주소」에 넣으세요.");
            let line = if ips_state.is_none() {
                line.value("찾는 중…", p.muted)
            } else {
                line.value("찾지 못함 — 네트워크 연결을 확인하세요", p.warn)
            };
            y += row_text(pc, cx, y, w, 0.0, &line);
        }
        for (i, addr) in ips.iter().enumerate() {
            if i > 0 && !self.other_ips {
                break;
            }
            divider(pc, cx, y, w);
            let ip = &addr.ip;
            // 어느 네트워크 주소인지 먼저 — 상대 컴퓨터와 같은 네트워크(같은 공유기)의 주소를 넣는다.
            let net = if addr.network.is_empty() {
                String::new()
            } else {
                format!("{} 주소입니다. ", addr.network)
            };
            let line = if i == 0 {
                Line::new("이 컴퓨터 주소", format!("{}이 주소를 상대 컴퓨터의 「상대 컴퓨터 주소」에 넣으세요.", net))
            } else if addr.virtual_adapter {
                Line::new("다른 주소", format!("{}다른 컴퓨터와 연결할 때는 보통 쓰지 않습니다.", net))
            } else {
                Line::new(
                    "다른 주소",
                    format!(
                        "{}상대 컴퓨터가 이 네트워크에 있으면 그 컴퓨터의 「상대 컴퓨터 주소」에 이 주소를 넣으세요.",
                        net
                    ),
                )
            };
            let copied = matches!(self.copied, Some((k, t)) if k == i && t.elapsed().as_secs_f32() < 2.0);
            if copied {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
            // 같은 동작은 같은 모양 — 다른 주소의 「복사」도 단추(접어 두어 늘 보이지는 않음, 첫 사용 점검 0.16.2).
            let variant = Variant::Default;
            let (hit, h) = buttons_row(
                pc,
                &format!("copy{}", i),
                cx,
                y,
                w,
                line.value(ip.clone(), p.ink),
                &[(if copied { "복사함" } else { "복사" }, None, variant)],
            );
            if hit.is_some() {
                ctx.copy_text(ip.clone());
                self.copied = Some((i, Instant::now()));
            }
            if i == 0 && ips.len() > 1 {
                // 「다른 주소 보기」 ↔ 「다른 주소 숨기기」 — 표시만 바꾸므로 링크, 첫 주소 설명 바로 아래.
                y += h - ROW_V + 2.0;
                let link = if self.other_ips { "다른 주소 숨기기" } else { "다른 주소 보기" };
                let lw = pc.btn_w(link, None);
                if pc.button("otherips", cx + PX - 14.0, y, lw, bh, link, None, Variant::Ghost, true) {
                    self.other_ips = !self.other_ips;
                }
                y += bh + ROW_V - 4.0;
            } else {
                y += h;
            }
        }
        y
    }

    /// 「Windows 방화벽」 줄. sharing_off = 공유를 끈 상태(규칙이 남아 있을 때만 불림). 줄 높이.
    fn firewall_row(&mut self, pc: &Pc<'_>, cx: f32, y: f32, w: f32, cfg: &AppConfig, sharing_off: bool) -> f32 {
        let p = pc.pal;
        let fw = *self.firewall_on.lock().unwrap();
        let desc = if sharing_off {
            format!(
                "클립보드 공유를 꺼도 포트 {}번 허용 규칙은 남아 있습니다. 쓰지 않으면 해제하세요(관리자 승인 필요).",
                cfg.clip_port
            )
        } else {
            format!(
                "상대 컴퓨터가 이 PC 로 보내려면 포트 {}번을 열어 두어야 합니다. 바꿀 때 관리자 승인이 필요합니다.",
                cfg.clip_port
            )
        };
        let line = Line::new("Windows 방화벽", desc);
        let (hit, h) = match fw {
            None => (None, row_text(pc, cx, y, w, 0.0, &line.value("확인 중…", p.muted))),
            Some(true) => buttons_row(pc, "fwoff", cx, y, w, line.value("허용됨", p.ink), &[("허용 해제", None, Variant::Default)]),
            Some(false) => buttons_row(pc, "fwon", cx, y, w, line.value("허용 규칙 없음", p.warn), &[("허용", None, Variant::Default)]),
        };
        if hit.is_some() {
            let failed = match fw {
                Some(false) => (!firewall::request_allow(cfg.clip_port))
                    .then_some("허용하지 못했습니다(취소했거나 관리자 승인이 막혔습니다). 회사 관리 PC 는 정책이 막을 수 있습니다."),
                Some(true) => (self.ask(
                    "방화벽 허용을 해제할까요?\r\n해제하면 상대 컴퓨터가 이 PC 로 클립보드를 보내지 못합니다.",
                    "Windows 방화벽",
                ) && !firewall::request_remove())
                .then_some("해제하지 못했습니다(취소했거나 관리자 승인이 막혔습니다)."),
                None => None,
            };
            if let Some(t) = failed {
                self.msg(t, "Windows 방화벽", MB_OK | MB_ICONWARNING);
            }
            let port = cfg.clip_port;
            spawn_check(&self.firewall_on, move || firewall::allowed_for_port(port));
        }
        h
    }

    fn toggle_prelogon(&mut self, on: bool, cfg: &AppConfig) {
        if on {
            if cfg.attachments.is_empty() {
                self.msg(
                    "먼저 「전환 관리」에서 채널 상자를 모니터 가장자리에 붙여 주세요. 그 배치를 로그인·잠금 화면에서도 씁니다.",
                    "로그인 전에도 실행",
                    MB_OK | MB_ICONINFORMATION,
                );
                return;
            }
            // 이전 판(C#)이 남아 있으면 두 판이 함께 전환한다 — 이 앱에서 바로 정리하고 이어서 켠다.
            let l = legacy::leftovers();
            if l.conflicts() || l.installed {
                *self.legacy.lock().unwrap() = Some(l);
                self.start_legacy_cleanup(cfg, true);
                return;
            }
            let modal = Modal::new("로그인 전에도 실행 — 켜는 중");
            let (step, fin, after) = (modal.step.clone(), modal.finished.clone(), modal.pre_logon_after.clone());
            let cfg2 = cfg.clone();
            std::thread::spawn(move || {
                *step.lock().unwrap() = "채널 배치 저장 중…".into();
                tray::write_logon_edges(&cfg2); // 머신 공유 엣지 설정 먼저
                *step.lock().unwrap() = "관리자 승인을 요청하는 중…\r\n승인 창이 뜨면 「예」를 눌러 주세요.".into();
                let ok = prelogon::request_enable(cfg2.my_host);
                *after.lock().unwrap() = Some(ok);
                *fin.lock().unwrap() = Some((
                    if ok {
                        "로그인 전에도 실행을 켰습니다.\r\n이제 PC 를 켠 직후의 로그인 화면과 잠금 화면에서도 가장자리로 밀면 넘어갑니다.\r\n\r\n리시버가 이 PC 에 꽂혀 있어야 합니다. 로그인 화면에서는 마우스를 한 번 움직여(키보드는 아무 키나 한 번) 깨운 뒤 미세요.".into()
                    } else {
                        "켜지 못했습니다(취소했거나 관리자 승인이 막혔습니다).\r\n회사 관리 PC 는 정책이 막을 수 있습니다.".into()
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
            let modal = Modal::new("로그인 전에도 실행 — 끄는 중");
            let (step, fin, after) = (modal.step.clone(), modal.finished.clone(), modal.pre_logon_after.clone());
            std::thread::spawn(move || {
                *step.lock().unwrap() = "관리자 승인을 요청하는 중…\r\n승인 창이 뜨면 「예」를 눌러 주세요.".into();
                let ok_msg = prelogon::request_disable();
                tray::remove_logon_edges();
                // 삭제 성공을 낙관하지 말고 실제 서비스 상태를 재조회.
                let still = prelogon::is_installed();
                let good = ok_msg && !still;
                *after.lock().unwrap() = Some(still);
                *fin.lock().unwrap() = Some((
                    if good {
                        "로그인 전에도 실행을 껐습니다.".into()
                    } else {
                        "끄지 못했습니다(취소했거나 관리자 승인이 막혔습니다).".into()
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
        // 글 길이·글자 크기에 맞춰 창 높이를 잡는다(글이 잘리지 않게, 최소 244).
        let body = match &fin {
            Some((text, _)) => text.clone(),
            None => step.clone(),
        };
        let measure = |s: &str, f: egui::FontId| ctx.fonts_mut(|fx| fx.layout(s.replace("\r\n", "\n"), f, Color32::WHITE, 400.0).size().y);
        let title_h = measure(&title, crate::paint::font(10.5, off, true));
        let body_h = measure(&body, crate::paint::font(9.0, off, false));
        let btn_h = (ctx.fonts_mut(|fx| fx.layout_no_wrap("가".into(), crate::paint::font(9.0, off, false), Color32::WHITE).size().y) + 12.0).max(32.0);
        let body_y = 16.0 + title_h + 12.0;
        let height = (body_y + body_h + 24.0 + btn_h + 12.0).max(244.0);
        let pos = center_on(ctx, parent, Vec2::new(440.0, height));
        let mut close = false;
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("lf-progress"),
            ViewportBuilder::default()
                .with_title(title.clone())
                .with_inner_size([440.0, height])
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
                pc.wrap(20.0, 16.0, 400.0, &title, 10.5, true, pal.ink);
                match &fin {
                    None => {
                        pc.wrap(20.0, body_y, 400.0, &step, 9.0, false, pal.muted);
                        // 진행 막대(Marquee).
                        let bar = pc.r(20.0, height - 12.0 - btn_h / 2.0 - 5.0, 400.0, 10.0);
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
                        pc.wrap(20.0, body_y, 400.0, text, 9.0, false, pal.ink);
                        if pc.button("ok", 330.0, height - 12.0 - btn_h, 90.0, btn_h, "확인", None, Variant::Default, true)
                            || ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            close = true;
                        }
                    }
                }
            },
        );
        if close {
            let after = *self.modal.as_ref().unwrap().pre_logon_after.lock().unwrap();
            let cleaned = self.modal.as_ref().unwrap().after_cleanup;
            self.modal = None;
            if cleaned {
                refresh_legacy(&self.legacy);
                self.apply(|_| {}); // 트레이가 설정을 다시 적용하며 가장자리 감시를 다시 켠다
            }
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
    fn diag_page(&mut self, pc: &Pc<'_>, cfg: &AppConfig, vw: f32) -> f32 {
        let p = pc.pal;
        let w = cw(vw);
        let x0 = 24.0;
        let bh = pc.btn_h();
        let mut y = page_header(
            pc,
            w,
            "진단",
            if CAPS.probe {
                "잘 안 될 때 기록을 남겨 원인을 찾고, 로그인 화면 전환이 이 PC 에서 되는지 미리 알아봅니다."
            } else {
                "잘 안 될 때 기록을 남겨 원인을 찾습니다."
            },
        );

        // 이전 판(C# 판)이 남아 있을 때만 — 「이전 판 지우기」. 머리 「이전 판」 · 제목 「남은 것」 · 주황 줄은 남은 것 목록만
        // (0.16.1 은 머리 · 제목 · 주황 줄이 모두 「이전 판이 남았다」를 되풀이했다 — 첫 사용 점검).
        let legacy_now = self.legacy.lock().unwrap().clone();
        if let Some(l) = legacy_now.filter(|l| l.any()) {
            y = section(pc, y, "이전 판");
            let slot = pc.card_begin();
            let top = y;
            let (hit, h) = buttons_row(
                pc,
                "legacy2",
                x0,
                y,
                w,
                Line::new("남은 것", LEGACY_WHAT).status(l.describe(), p.warn),
                &[("이전 판 지우기", None, Variant::Default)],
            );
            if hit.is_some() {
                self.start_legacy_cleanup(cfg, false);
            }
            y += h;
            pc.card_end(slot, x0, top, w, y - top);
            y += GROUP_GAP;
        }

        // 기록 — 남기기 설정과 파일 다루기를 한곳에.
        y = section(pc, y, "기록");
        let slot = pc.card_begin();
        let top = y;
        let (hit, h) = toggle_row(
            pc,
            "diaglog",
            x0,
            y,
            w,
            Line::new("진단 기록 남기기", "가장자리 감지와 전환 기록을 아래 「기록 폴더」에 남깁니다."),
            cfg.diag_log_enabled,
        );
        if hit {
            self.apply(|c| c.diag_log_enabled = !c.diag_log_enabled);
        }
        y += h;
        // 상세 기록은 로그인 화면 워커용 — 로그인 전에도 실행이 있는 OS 에서만.
        // 0.16.2: 위 스위치의 하위가 아니다(코드 확인). 진단 기록 남기기(diagLog)는 트레이가 설치 폴더 logs 에 쓰는 기록을,
        // 상세 기록(debug → logon-edges.txt)은 로그인 전 실행 서비스(SYSTEM)가 로그인 화면 기록(desktop-probe.log)에
        // 커서 위치를 더 쓰는지를 정한다 — 앞을 꺼도 상세 기록은 남는다. 그래서 들여 쓰거나 흐리게 하지 않고, 이름과
        // 설명에 따로 동작함과 남는 곳을 적는다.
        if CAPS.prelogon {
            divider(pc, x0, y, w);
            let (hit, h) = toggle_row(
                pc,
                "debug",
                x0,
                y,
                w,
                Line::new(
                    "로그인 화면 상세 기록",
                    "「로그인 전에도 실행」을 켰는데도 로그인 화면에서 넘어가지 않을 때 켜세요. 로그인 화면의 커서 위치까지 「로그인 화면 테스트」의 「결과 보기」 기록에 남깁니다. 위 「진단 기록 남기기」와 따로 켜고 끕니다.",
                ),
                cfg.debug_mode,
            );
            if hit {
                self.apply(|c| c.debug_mode = !c.debug_mode);
            }
            y += h;
        }
        divider(pc, x0, y, w);
        let (hit, h) = buttons_row(
            pc,
            "logdir",
            x0,
            y,
            w,
            Line::new("기록 폴더", "LogiFlow 가 설치된 폴더 안 logs 폴더에 있습니다."),
            &[("폴더 열기", None, Variant::Ghost)],
        );
        if hit.is_some() {
            let dir = log_dir();
            let _ = std::fs::create_dir_all(&dir);
            sys::util::open_with("explorer.exe", &dir);
        }
        y += h;
        if has_logs() {
            divider(pc, x0, y, w);
            let (hit, h) = buttons_row(
                pc,
                "logdel",
                x0,
                y,
                w,
                Line::new("기록 지우기", "지금까지 남긴 기록 파일을 지웁니다."),
                &[("지우기", None, Variant::Default)],
            );
            if hit.is_some() && self.ask("기록 파일을 모두 지울까요?\r\n지운 기록은 되살릴 수 없습니다.", "기록 지우기") {
                let mut left = 0;
                if let Ok(rd) = std::fs::read_dir(log_dir()) {
                    for e in rd.flatten() {
                        if std::fs::remove_file(e.path()).is_err() {
                            left += 1;
                        }
                    }
                }
                if left > 0 {
                    self.msg(
                        &format!("{}개 파일은 지금 쓰는 중이라 지우지 못했습니다. 잠시 뒤 다시 해 보세요.", left),
                        "기록 지우기",
                        MB_OK | MB_ICONWARNING,
                    );
                }
            }
            y += h;
        }
        pc.card_end(slot, x0, top, w, y - top);
        y += GROUP_GAP;

        // 로그인 화면 테스트(읽기 전용 — 마우스·키보드를 넘기지 않음). Windows 만.
        if !CAPS.probe {
            return y + 10.0;
        }
        self.test_y = y;
        y = section(pc, y, "로그인 화면 테스트");
        let slot = pc.card_begin();
        let top = y;
        let tx = x0 + PX;
        let tw = w - 2.0 * PX;
        let mut yy = y + ROW_V;
        yy += pc.wrap(
            tx,
            yy,
            tw,
            "「로그인 전에도 실행」을 켜기 전에, 이 PC 가 로그인·잠금 화면에서도 가장자리 전환을 할 수 있는지 알아봅니다. 마우스·키보드는 넘기지 않고 커서 위치만 읽습니다. 실행할 때 관리자 승인이 필요합니다.",
            DESC,
            false,
            p.muted,
        );
        yy += 8.0;
        yy += pc.wrap(
            tx,
            yy,
            tw,
            "1. 「테스트 실행」을 누릅니다.\n2. 곧바로 Win+L 로 잠금 화면에 가서 마우스를 20~30초 움직입니다.\n3. 다시 로그인해 「결과 보기」를 누릅니다.",
            DESC,
            false,
            p.ink,
        );
        yy += 12.0;
        let run_w = pc.btn_w("테스트 실행", None);
        if pc.button("prun", tx, yy, run_w, bh, "테스트 실행", None, Variant::Default, true) {
            let ok = probe::request_install_run();
            if ok {
                *self.probe_installed.lock().unwrap() = Some(true);
            }
            self.msg(
                if ok {
                    "테스트를 시작했습니다(약 2분 동안 기록합니다).\r\n\r\n1. 지금 Win+L 로 잠금 화면에 가서 마우스를 20~30초 움직이세요.\r\n2. 다시 로그인해 「결과 보기」를 누르세요."
                } else {
                    "테스트를 시작하지 못했습니다(취소했거나 관리자 승인이 막혔습니다).\r\n회사 관리 PC 정책이 막으면 여기서 멈추며, 그 자체가 「이 PC 에서는 안 됨」이라는 뜻입니다."
                },
                "로그인 화면 테스트",
                MB_OK | if ok { MB_ICONINFORMATION } else { MB_ICONWARNING },
            );
        }
        // 결과는 보기만 하므로 링크.
        let res_w = pc.btn_w("결과 보기", None);
        if pc.button("presult", tx + run_w + GAP, yy, res_w, bh, "결과 보기", None, Variant::Ghost, true) {
            self.show_probe_result(cfg);
        }
        yy += bh + ROW_V;
        y = yy;
        // 지우는 동작은 맨 아래 따로 — 테스트 도구가 설치돼 있을 때만.
        if *self.probe_installed.lock().unwrap() == Some(true) {
            divider(pc, x0, y, w);
            let (hit, h) = buttons_row(
                pc,
                "premove",
                x0,
                y,
                w,
                Line::new(
                    "테스트 도구 지우기",
                    "「테스트 실행」 때 이 PC 에 설치한 테스트 도구를 지웁니다. 결과 기록은 남습니다. 관리자 승인이 필요합니다.",
                ),
                &[("지우기", None, Variant::Default)],
            );
            if hit.is_some() && self.ask("로그인 화면 테스트 도구를 지울까요?\r\n결과 기록은 남습니다.", "테스트 도구 지우기") {
                if !probe::request_uninstall() {
                    self.msg(
                        "지우지 못했습니다(취소했거나 관리자 승인이 막혔습니다).",
                        "테스트 도구 지우기",
                        MB_OK | MB_ICONWARNING,
                    );
                }
                spawn_check(&self.probe_installed, probe::is_installed);
            }
            y += h;
        }
        pc.card_end(slot, x0, top, w, y - top);
        y + 28.0
    }

    /// 「결과 보기」 — 기록을 읽어 쉬운 말로 판정만 보여 준다. 되면 바로 켤지 묻고, 아니면 원본 기록을 열지 묻는다.
    fn show_probe_result(&mut self, cfg: &AppConfig) {
        let path = paths::probe_log_path();
        let verdict = read_lines(&path).map(|l| probe_result::verdict(&l)).unwrap_or(ProbeVerdict::NotRun);
        let title = "로그인 화면 테스트";
        let text = match verdict {
            ProbeVerdict::NotRun => {
                self.msg(
                    "아직 테스트 결과가 없습니다.\r\n「테스트 실행」을 누른 뒤 잠금 화면에서 마우스를 움직여 보세요.",
                    title,
                    MB_OK | MB_ICONINFORMATION,
                );
                return;
            }
            ProbeVerdict::Works => {
                let installed = *self.prelogon_installed.lock().unwrap();
                if installed {
                    self.msg(
                        "이 PC 는 로그인·잠금 화면에서도 가장자리 전환을 할 수 있습니다.\r\n「로그인 전에도 실행」은 이미 켜져 있습니다.",
                        title,
                        MB_OK | MB_ICONINFORMATION,
                    );
                } else if self.ask(
                    "이 PC 는 로그인·잠금 화면에서도 가장자리 전환을 할 수 있습니다.\r\n\r\n지금 「로그인 전에도 실행」을 켤까요?",
                    title,
                ) {
                    self.page = 1;
                    self.offset = 0.0;
                    self.toggle_prelogon(true, cfg);
                }
                return;
            }
            ProbeVerdict::NoSecureScreen => "테스트는 실행됐지만 잠금 화면에 간 기록이 없습니다.\r\n「테스트 실행」 뒤 2분 안에 Win+L 로 잠금 화면에 가서 마우스를 움직여 주세요.",
            ProbeVerdict::Blocked => "이 PC 에서는 잠금·로그인 화면의 커서를 읽지 못했습니다.\r\n회사 보안 정책 때문일 수 있으며, 「로그인 전에도 실행」을 켜도 로그인 화면에서는 넘어가지 않을 수 있습니다.",
            ProbeVerdict::NoMovement => "잠금 화면에는 갔지만 커서 움직임이 기록되지 않았습니다.\r\n잠금 화면에서 마우스를 20~30초 움직이며 다시 해 보세요.",
        };
        if self.ask(&format!("{}\r\n\r\n원본 기록 파일도 열까요?", text), title) {
            sys::util::open_with("notepad.exe", &path);
        }
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
        let (page, dark, off, bottom, variant) = sn.steps[sn.idx];
        if sn.idx == 0 && sn.frames == 1 {
            // 글자 실측(0.16.2 — Mac 「작게」 설명 글 크기 확인): 화면 배율 · 단계별 설명 글 em · 줄 높이 · 한글 한 자 폭.
            let ppp = ctx.pixels_per_point();
            let mut m = format!("pixels_per_point={}\n", ppp);
            for o in [-1, 1, 3] {
                let f = crate::paint::font(DESC, o, false);
                let g = ui.painter().layout_no_wrap("가나다라마바사아자차".into(), f.clone(), Color32::WHITE);
                let l = ui.painter().layout_no_wrap("LogiFlow".into(), f.clone(), Color32::WHITE);
                m.push_str(&format!(
                    "font{} desc_em_pt={:.2} row_h={:.2} hangul_advance={:.2} latin_LogiFlow_w={:.2}\n",
                    o,
                    f.size,
                    g.size().y,
                    g.size().x / 10.0,
                    l.size().x
                ));
            }
            let _ = std::fs::create_dir_all(&sn.dir);
            let _ = std::fs::write(sn.dir.join("metrics.txt"), m);
        }
        if sn.frames == 0 {
            self.page = page;
            self.offset = if bottom { 1.0e6 } else { 0.0 };
            self.detail = variant == 2;
            self.other_ips = variant == 6;
            let mut s = self.shared.lock().unwrap();
            s.config.theme = if dark { ThemeMode::Dark } else { ThemeMode::Light };
            s.config.font_offset = off;
            s.config.clip_enabled = variant != 1;
            // 기기를 못 찾은 모양(0.16.1 빈 화면 안내) — 이 단계만 기기 목록을 비웠다가 다음 단계에서 되돌린다.
            if variant == 5 {
                if sn.stash.is_empty() {
                    sn.stash = std::mem::take(&mut s.devices);
                }
            } else if !sn.stash.is_empty() {
                s.devices = std::mem::take(&mut sn.stash);
            }
            drop(s);
            // 공유를 끈 화면은 「허용 규칙이 남아 있는」 모양으로 찍는다(방화벽 줄이 접히지 않는지 보려고).
            if variant == 1 {
                *self.firewall_on.lock().unwrap() = Some(true);
            } else if variant == 0 {
                *self.firewall_on.lock().unwrap() = Some(false);
            }
            // 이전 판(C#)이 남은 모양 — 정리 단추·안내가 보이는지.
            *self.legacy.lock().unwrap() = Some(if variant == 4 {
                legacy::Leftovers {
                    service: true,
                    machine_dir: true,
                    ..Default::default()
                }
            } else {
                legacy::Leftovers::default()
            });
        }
        // 펼침 등으로 콘텐츠 높이가 바뀐 뒤 한 번 더 맞춘다(첫 프레임의 맨 아래는 옛 높이 기준이라).
        if sn.frames == 3 {
            if bottom {
                self.offset = 1.0e6;
            } else if variant == 3 || variant == 6 {
                self.offset = (self.content_h / 2.0 - 300.0).max(0.0);
            }
        }
        let sn = self.snapshot.as_mut().unwrap();
        sn.frames += 1;
        // 백그라운드 조회(방화벽·테스트 작업)가 끝날 시간을 조금 준다.
        if sn.frames == 8 && !sn.requested {
            sn.requested = true;
            let name = format!(
                "{}-{}-font{}{}{}.png",
                ["switch", "settings", "diag"][page],
                if dark { "dark" } else { "light" },
                off,
                if bottom { "-bottom" } else { "" },
                ["", "-clipoff", "-detail", "-mid", "-legacy", "-nodevices", "-mid-otherips"][variant as usize]
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
            after_cleanup: false,
            started: Instant::now(),
        }
    }
}

fn center_on(ctx: &egui::Context, parent: Rect, size: Vec2) -> Pos2 {
    let outer = ctx.input(|i| i.viewport().outer_rect).unwrap_or(parent);
    Pos2::new(outer.center().x - size.x / 2.0, outer.center().y - size.y / 2.0)
}

#[allow(dead_code)]
fn unused(_: Color32) {}
