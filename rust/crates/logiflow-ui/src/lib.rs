//! LogiFlow 설정 창(egui) — `LogiFlowRs.exe --ui`. 트레이가 설정 열 때만 띄우고, 닫으면 끝난다.
//! 창 크기·토큰은 C# 판과 같고, rs.3 에서 첫 사용 점검 20건만큼 배치·문구를 고쳤다(app.rs 머리 주석).

#![allow(clippy::too_many_arguments)]

pub mod canvas;
pub mod paint;

#[cfg(any(windows, target_os = "macos"))]
mod app;
#[cfg(any(windows, target_os = "macos"))]
mod platform;

/// 화면 캡처(`--ui-snapshot`) 공용 — 시험 자료와 찍을 차례. OS 마다 다른 것: 첫 모니터 이름 · 기기 경로 모양 ·
/// 이전 판(C#) 남은 모양(Windows 만).
#[cfg(any(windows, target_os = "macos"))]
mod snap {
    use crate::app::{SettingsApp, Shared, SnapStep, Snapshot};
    use logiflow_core::config::{AppConfig, EdgeAttachment};
    use logiflow_core::edge::HandoffEdge;
    use logiflow_core::scan::ScanResult;
    use std::sync::{Arc, Mutex};

    pub(crate) fn setup(first_monitor: Option<String>, path_of: fn(&str) -> String, with_legacy: bool) -> (Arc<Mutex<Shared>>, Vec<SnapStep>) {
        let mut cfg = AppConfig::default();
        cfg.set_alias(0, &crate::os_text("이 PC"));
        cfg.set_alias(1, "노트북");
        cfg.clip_peer_host = "192.168.0.20".into();
        cfg.clip_enabled = true; // 펼친 모양(주소·암호문구·방화벽 줄)도 찍는다. 암호문구는 비워 「아직 없음」 모양.
        if let Some(m) = first_monitor {
            cfg.attachments.push(EdgeAttachment {
                channel: 1,
                monitor_device_name: m,
                edge: HandoffEdge::Right,
            });
        }
        let dev = |name: &str, idx: u8, ch: u8| ScanResult {
            device_name: name.into(),
            vid_pid: "046D:C548".into(),
            path: path_of(&idx.to_string()),
            connection_type: format!("리시버 뒤(index {})", idx),
            device_index: idx,
            change_host_supported: true,
            change_host_index: ch,
            host_count: 3,
            current_host: 0,
            ..Default::default()
        };
        // 전환이 안 되는 장치(리시버 자체) 하나 — 「자세히 보기」 모양을 찍으려고.
        let receiver = ScanResult {
            device_name: "USB Receiver".into(),
            vid_pid: "046D:C548".into(),
            path: path_of("r"),
            connection_type: "직접(USB/BT)".into(),
            device_index: 0xFF,
            ..Default::default()
        };
        let shared = Arc::new(Mutex::new(Shared {
            config: cfg,
            devices: vec![dev("MX Master 3S", 1, 0x0A), dev("MX Keys S", 2, 0x09), receiver],
            clip_running: false, // 암호문구가 비었으니 트레이도 공유를 시작하지 않은 상태
            ..Default::default()
        }));
        // 3화면 × 글자 3단(작게 −1 · 보통 +1 · 크게 +3) × 위/아래(라이트) + 보통 다크 위/아래 + 클립보드 꺼진 설정 화면.
        let mut steps: Vec<SnapStep> = Vec::new();
        for off in [-1, 1, 3] {
            for page in 0..3 {
                steps.push((page, false, off, false, 0));
                steps.push((page, false, off, true, 0));
            }
        }
        for page in 0..3 {
            steps.push((page, true, 1, false, 0));
            steps.push((page, true, 1, true, 0));
        }
        steps.push((1, false, 1, false, 1)); // 클립보드 공유 끔(접힘)
        steps.push((1, false, 1, true, 1));
        for off in [-1, 1, 3] {
            steps.push((1, false, off, false, 3)); // 설정 가운데(클립보드 공유 위쪽)
        }
        if with_legacy {
            // 기존 LogiFlow(이전 판)가 남은 모양 — 전환 관리 카드 안 알림 · 진단 카드.
            steps.push((0, false, 1, false, 4));
            steps.push((0, false, 3, false, 4));
            steps.push((2, false, 1, false, 4));
        }
        steps.push((0, false, 1, true, 2)); // 기기 자세히 보기
        steps.push((0, false, 3, true, 2));
        steps.push((0, false, 1, false, 5)); // 기기를 못 찾았을 때(빈 화면 안내)
        steps.push((0, true, -1, false, 5));
        (shared, steps)
    }

    pub(crate) fn app(shared: Arc<Mutex<Shared>>, version: String, dir: std::path::PathBuf, steps: Vec<SnapStep>) -> SettingsApp {
        let mut app = SettingsApp::new(shared, None, version);
        app.snapshot = Some(Snapshot {
            dir,
            steps,
            idx: 0,
            frames: 0,
            requested: false,
            done: Default::default(),
            stash: Vec::new(),
        });
        app
    }
}

/// 화면 글을 이 OS 말로(Mac: 「이 PC」 → 「이 Mac」 등). Windows 는 그대로.
#[cfg(target_os = "macos")]
pub(crate) fn os_text(s: &str) -> std::borrow::Cow<'_, str> {
    logiflow_mac::dialog::localize(s)
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn os_text(s: &str) -> std::borrow::Cow<'_, str> {
    std::borrow::Cow::Borrowed(s)
}

#[cfg(windows)]
pub use win::*;

#[cfg(windows)]
mod win {
    use crate::app::{SettingsApp, Shared};
    use egui::{FontData, FontDefinitions, FontFamily};
    use logiflow_core::cache;
    use logiflow_core::config::AppConfig;
    use logiflow_win::wnd::{self, Handler, Hwnd};
    use logiflow_win::{ipc, paths, tray, util};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::rc::Rc;
    use std::sync::{Arc, Mutex, OnceLock};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SendMessageW, ICON_BIG, ICON_SMALL, MB_ICONWARNING, MB_OK, SM_CXICON, SM_CXSMICON, WM_COPYDATA, WM_SETICON,
    };

    static IPC_HWND: OnceLock<Hwnd> = OnceLock::new();

    pub(crate) fn ipc_window() -> HWND {
        IPC_HWND.get().copied().unwrap_or_default().get()
    }

    pub(crate) fn window_hwnd(frame: &eframe::Frame) -> Option<HWND> {
        match frame.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
            _ => None,
        }
    }

    /// 실행 파일에 내장된 앱 아이콘(리소스 1)을 창에 — 트레이·작업 표시줄과 같은 아이콘.
    pub(crate) fn set_window_icon(h: HWND) {
        unsafe {
            let big = logiflow_win::notify::app_icon(GetSystemMetrics(SM_CXICON));
            let small = logiflow_win::notify::app_icon(GetSystemMetrics(SM_CXSMICON));
            SendMessageW(h, WM_SETICON, Some(WPARAM(ICON_BIG as usize)), Some(LPARAM(big.0 as isize)));
            SendMessageW(h, WM_SETICON, Some(WPARAM(ICON_SMALL as usize)), Some(LPARAM(small.0 as isize)));
        }
    }

    /// 32px 아이콘(logiflow.ico 안의 PNG) — 창이 뜨는 순간부터 egui 기본 아이콘 대신.
    fn icon_data() -> Option<egui::IconData> {
        let ico: &[u8] = include_bytes!("../../../../LogiFlow/Resources/logiflow.ico");
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        for i in 0..count {
            let e = &ico[6 + 16 * i..22 + 16 * i];
            if e[0] == 32 {
                let size = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
                let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
                let png = ico.get(off..off + size)?;
                let mut dec = png::Decoder::new(std::io::Cursor::new(png));
                dec.set_transformations(png::Transformations::EXPAND);
                let mut r = dec.read_info().ok()?;
                let mut buf = vec![0; r.output_buffer_size()?];
                let info = r.next_frame(&mut buf).ok()?;
                if info.color_type != png::ColorType::Rgba {
                    return None;
                }
                buf.truncate(info.buffer_size());
                return Some(egui::IconData {
                    rgba: buf,
                    width: info.width,
                    height: info.height,
                });
            }
        }
        None
    }

    /// Segoe UI(보통·굵게) + 맑은 고딕(한글) — WinForms 글꼴 연결과 같은 조합. 없으면 egui 기본.
    fn fonts() -> FontDefinitions {
        let mut defs = FontDefinitions::default();
        let dir = std::path::PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into())).join("Fonts");
        let mut load = |key: &str, file: &str| -> bool {
            match std::fs::read(dir.join(file)) {
                Ok(b) => {
                    defs.font_data.insert(key.into(), Arc::new(FontData::from_owned(b)));
                    true
                }
                Err(_) => false,
            }
        };
        let seg = load("segoe", "segoeui.ttf");
        let segb = load("segoe-bold", "segoeuib.ttf");
        let mal = load("malgun", "malgun.ttf");
        let malb = load("malgun-bold", "malgunbd.ttf");
        let base: Vec<String> = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
        let mut regular = Vec::new();
        if seg {
            regular.push("segoe".to_string());
        }
        if mal {
            regular.push("malgun".to_string());
        }
        regular.extend(base.iter().cloned());
        let mut bold = Vec::new();
        if segb {
            bold.push("segoe-bold".to_string());
        }
        if malb {
            bold.push("malgun-bold".to_string());
        }
        bold.extend(regular.iter().cloned());
        defs.families.insert(FontFamily::Proportional, regular);
        defs.families.insert(FontFamily::Name("bold".into()), bold);
        defs
    }

    /// `--ui-snapshot=<폴더>` — CI 화면 캡처(트레이 없이 시험 자료로 3화면 × 글자 3단 × 라이트, 보통 다크).
    pub fn run_snapshot(version: &str, dir: &str) {
        let first = logiflow_win::monitors::all().first().map(|m| m.name.clone());
        let (shared, steps) = crate::snap::setup(first, |k| format!(r"\\?\hid#vid_046d&pid_c548&mi_02&col02#{}", k), true);
        let viewport = egui::ViewportBuilder::default().with_title("LogiFlow").with_inner_size([820.0, 660.0]);
        let options = eframe::NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        };
        let version = version.to_string();
        let dir = std::path::PathBuf::from(dir);
        let _ = eframe::run_native(
            "LogiFlow",
            options,
            Box::new(move |cc| {
                cc.egui_ctx.set_fonts(fonts());
                Ok(Box::new(crate::snap::app(shared, version, dir, steps)))
            }),
        );
    }

    /// `--ui` 진입점.
    pub fn run(version: &str) {
        wnd::init_dispatcher();
        let tray = ipc::find_tray().map(Hwnd::of);
        if tray.is_none() {
            util::message_box(
                None,
                "LogiFlow 트레이가 실행 중이 아닙니다. LogiFlow 를 먼저 실행해 주세요.",
                "LogiFlow",
                MB_OK | MB_ICONWARNING,
            );
            return;
        }
        // 트레이 상태가 오기 전에도 바로 보이게 파일에서 먼저 읽는다.
        let shared = Arc::new(Mutex::new(Shared {
            config: tray::read_config_direct(),
            devices: cache::load(&paths::devices_path()),
            ..Default::default()
        }));
        let ctx_cell: Rc<std::cell::RefCell<Option<egui::Context>>> = Rc::new(std::cell::RefCell::new(None));
        {
            let shared = shared.clone();
            let ctx_cell = ctx_cell.clone();
            let handler: Handler = Rc::new(move |_h, msg, _w, lp| {
                if msg != WM_COPYDATA {
                    return None;
                }
                let (kind, text) = ipc::read(lp);
                {
                    let mut s = shared.lock().unwrap();
                    match kind {
                        ipc::STATE => {
                            let st = ipc::decode_state(&text);
                            s.config = AppConfig::from_plain_text(&st.config_plain);
                            let lines: Vec<String> = st.devices.lines().map(|l| l.to_string()).collect();
                            s.devices = cache::parse(&lines);
                            s.scanning = st.scanning;
                            s.clip_running = st.clip_running;
                        }
                        ipc::SWITCH_RESULT => s.switch_result = Some(text == "1"),
                        ipc::ACTIVATE => s.activate = true,
                        _ => {}
                    }
                }
                if let Some(c) = ctx_cell.borrow().as_ref() {
                    c.request_repaint();
                }
                Some(LRESULT(1))
            });
            if let Some(h) = wnd::message_window(ipc::UI_IPC_CLASS, handler) {
                let _ = IPC_HWND.set(Hwnd::of(h));
            }
        }

        let mut viewport = egui::ViewportBuilder::default()
            .with_title("LogiFlow")
            .with_inner_size([820.0, 660.0])
            .with_min_inner_size([760.0, 600.0]);
        if let Some(icon) = icon_data() {
            viewport = viewport.with_icon(icon);
        }
        let options = eframe::NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        };
        let version = version.to_string();
        let r = eframe::run_native(
            "LogiFlow",
            options,
            Box::new(move |cc| {
                cc.egui_ctx.set_fonts(fonts());
                *ctx_cell.borrow_mut() = Some(cc.egui_ctx.clone());
                Ok(Box::new(SettingsApp::new(shared, tray, version)))
            }),
        );
        if let Err(e) = r {
            // 그래픽 초기화 실패 등 — 조용히 사라지지 않게 알린다.
            util::message_box(None, &format!("설정 창을 열지 못했습니다.\r\n{}", e), "LogiFlow", MB_OK | MB_ICONWARNING);
        }
    }
}

#[cfg(target_os = "macos")]
pub use mac::*;

/// Mac 설정 창 — `LogiFlowRs.app/Contents/MacOS/LogiFlow --ui`. 메뉴 막대 상주 앱이 열 때만 띄우고 닫으면 끝난다.
/// Windows 와 같은 창·같은 3화면(공용 `app`), 상주 앱과는 Unix 소켓 IPC.
#[cfg(target_os = "macos")]
mod mac {
    use crate::app::{SettingsApp, Shared};
    use egui::{FontData, FontDefinitions, FontFamily};
    use logiflow_core::cache;
    use logiflow_core::config::AppConfig;
    use logiflow_mac::dialog::{HWND, MB_ICONWARNING, MB_OK};
    use logiflow_mac::wnd::Hwnd;
    use logiflow_mac::{ipc, paths, tray, util};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use skrifa::MetadataProvider;
    use std::sync::{Arc, Mutex, OnceLock};

    static IPC_ID: OnceLock<Hwnd> = OnceLock::new();

    /// 이 설정 창의 IPC 번호(프로세스 번호) — 상주 앱이 상태를 보낼 곳.
    pub(crate) fn ipc_window() -> HWND {
        IPC_ID.get().copied().unwrap_or_default().get()
    }

    /// winit 이 만든 NSView 주소(대화 상자·메뉴 위치 계산용).
    pub(crate) fn window_hwnd(frame: &eframe::Frame) -> Option<HWND> {
        match frame.window_handle().ok()?.as_raw() {
            RawWindowHandle::AppKit(h) => Some(HWND(h.ns_view.as_ptr() as isize)),
            _ => None,
        }
    }

    /// Dock 아이콘은 앱 번들 아이콘(AppIcon.icns)을 쓴다.
    pub(crate) fn set_window_icon(_h: HWND) {}

    /// 글꼴 모음(.ttc)에서 굵기가 가장 가까운 바른 글꼴(기울임 아님) 번호.
    fn face_by_weight(bytes: &[u8], want: f32) -> u32 {
        let mut best = (f32::MAX, 0u32);
        for i in 0..64u32 {
            let Ok(f) = skrifa::FontRef::from_index(bytes, i) else {
                break;
            };
            let a = f.attributes();
            if a.style != skrifa::attribute::Style::Normal {
                continue;
            }
            let d = (a.weight.value() - want).abs();
            if d < best.0 {
                best = (d, i);
            }
        }
        best.1
    }

    /// 시스템 한글 글꼴(Apple SD Gothic Neo 보통·굵게)로 한글·라틴 모두 — 없으면 egui 기본.
    fn fonts() -> FontDefinitions {
        let mut defs = FontDefinitions::default();
        let base: Vec<String> = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
        let mut regular = Vec::new();
        let mut bold = Vec::new();
        if let Ok(b) = std::fs::read("/System/Library/Fonts/AppleSDGothicNeo.ttc") {
            let (ri, bi) = (face_by_weight(&b, 400.0), face_by_weight(&b, 700.0));
            let blob: Arc<Vec<u8>> = Arc::new(b);
            defs.font_data.insert("sdgothic".into(), Arc::new(FontData::from_blob(blob.clone(), ri)));
            defs.font_data.insert("sdgothic-bold".into(), Arc::new(FontData::from_blob(blob, bi)));
            regular.push("sdgothic".to_string());
            bold.push("sdgothic-bold".to_string());
        }
        regular.extend(base.iter().cloned());
        bold.extend(regular.iter().cloned());
        defs.families.insert(FontFamily::Proportional, regular);
        defs.families.insert(FontFamily::Name("bold".into()), bold);
        defs
    }

    /// `--ui-snapshot=<폴더>` — CI 화면 캡처(상주 앱 없이 시험 자료로). Windows 와 같은 차례에서 이전 판(C#) 모양만 뺀다.
    pub fn run_snapshot(version: &str, dir: &str) {
        let first = logiflow_mac::monitors::all().first().map(|m| m.name.clone());
        let (shared, steps) = crate::snap::setup(first, |k| format!("loc=14200000:up=FF00:u=0002:pid=C548:id=1000{}", k), false);
        let viewport = egui::ViewportBuilder::default().with_title("LogiFlow").with_inner_size([820.0, 660.0]);
        let options = eframe::NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        };
        let version = version.to_string();
        let dir = std::path::PathBuf::from(dir);
        let _ = eframe::run_native(
            "LogiFlow",
            options,
            Box::new(move |cc| {
                cc.egui_ctx.set_fonts(fonts());
                Ok(Box::new(crate::snap::app(shared, version, dir, steps)))
            }),
        );
    }

    /// `--ui` 진입점.
    pub fn run(version: &str) {
        let tray = ipc::find_tray().map(Hwnd::of);
        if tray.is_none() {
            util::message_box(
                None,
                "LogiFlow 가 실행 중이 아닙니다. 응용 프로그램 폴더에서 LogiFlow 를 먼저 여세요.",
                "LogiFlow",
                MB_OK | MB_ICONWARNING,
            );
            return;
        }
        // 상주 앱 상태가 오기 전에도 바로 보이게 파일에서 먼저 읽는다.
        let shared = Arc::new(Mutex::new(Shared {
            config: tray::read_config_direct(),
            devices: cache::load(&paths::devices_path()),
            ..Default::default()
        }));
        let ctx_cell: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
        let pid = std::process::id();
        let sock = paths::ui_socket(pid);
        {
            let shared = shared.clone();
            let ctx_cell = ctx_cell.clone();
            let ok = ipc::listen(sock.clone(), move |kind, text| {
                {
                    let mut s = shared.lock().unwrap();
                    match kind {
                        ipc::STATE => {
                            let st = ipc::decode_state(&text);
                            s.config = AppConfig::from_plain_text(&st.config_plain);
                            let lines: Vec<String> = st.devices.lines().map(|l| l.to_string()).collect();
                            s.devices = cache::parse(&lines);
                            s.scanning = st.scanning;
                            s.clip_running = st.clip_running;
                            s.secret_note = (!st.secret_note.is_empty()).then_some(st.secret_note);
                        }
                        ipc::SWITCH_RESULT => s.switch_result = Some(text == "1"),
                        ipc::ACTIVATE => s.activate = true,
                        _ => {}
                    }
                }
                if let Some(c) = ctx_cell.lock().unwrap().as_ref() {
                    c.request_repaint();
                }
            });
            if ok {
                let _ = IPC_ID.set(Hwnd(pid as isize));
            }
        }

        let viewport = egui::ViewportBuilder::default()
            .with_title("LogiFlow")
            .with_inner_size([820.0, 660.0])
            .with_min_inner_size([760.0, 600.0]);
        let options = eframe::NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        };
        let version = version.to_string();
        let r = eframe::run_native(
            "LogiFlow",
            options,
            Box::new(move |cc| {
                cc.egui_ctx.set_fonts(fonts());
                *ctx_cell.lock().unwrap() = Some(cc.egui_ctx.clone());
                Ok(Box::new(SettingsApp::new(shared, tray, version)))
            }),
        );
        let _ = std::fs::remove_file(&sock);
        if let Err(e) = r {
            util::message_box(None, &format!("설정 창을 열지 못했습니다.\n{}", e), "LogiFlow", MB_OK | MB_ICONWARNING);
        }
    }
}
