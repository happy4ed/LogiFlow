//! 트레이 상주 앱 — C# `TrayAppContext` 이식(+ MainForm 의 기기 변화 감지). 설정·스캔·가장자리 감시·
//! 전환·기기 캐시·클립보드의 단일 주인. 설정 창은 `--ui` 별도 프로세스(열 때만)로 IPC 를 통해 붙는다.

use crate::clipboard::{self, ClipEvents};
use crate::dpapi;
use crate::edge_watcher;
use crate::glow_win;
use crate::hid::WinHidBackend;
use crate::ipc;
use crate::legacy;
use crate::monitors;
use crate::notify::{self, MenuItem, WM_TRAY};
use crate::paths;
use crate::util::{exe_dir, spawn_self, tick_ms, wide};
use crate::wnd::{self, Handler, Hwnd};
use crate::{autostart, workers};
use logiflow_core::cache;
use logiflow_core::config::AppConfig;
use logiflow_core::log::{write_utf8_bom, DiagLogger};
use logiflow_core::logon_edges;
use logiflow_core::scan::{self, ScanResult};
use logiflow_core::switcher;
use std::cell::RefCell;
use std::process::Child;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_DEVICECHANGE: u32 = 0x0219;
const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
const WTS_SESSION_LOCK: usize = 7;
const WTS_SESSION_UNLOCK: usize = 8;
const ID_SETTINGS: u32 = 1;
const ID_RESCAN: u32 = 2;
const ID_EXIT: u32 = 3;
const ID_CHANNEL: u32 = 1000;

struct Tray {
    log: Arc<DiagLogger>,
    config: AppConfig,
    last_results: Vec<ScanResult>,
    scanning: bool,
    signature: Arc<Mutex<String>>,
    last_device_change: u64,
    hwnd: HWND,
    ui_hwnd: Option<Hwnd>,
    ui_child: Option<Child>,
    exiting: bool,
}

thread_local! {
    static T: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Tray) -> R) -> Option<R> {
    T.with(|t| t.borrow_mut().as_mut().map(f))
}

fn save_config(cfg: &AppConfig) {
    let _ = dpapi::with_codec(|c| cfg.save(&paths::config_path(), Some(c)));
}

fn load_config() -> AppConfig {
    dpapi::with_codec(|c| AppConfig::load(&paths::config_path(), Some(c)))
}

/// 트레이 진입점(인자 없음). 단일 인스턴스.
pub fn run() {
    let name = wide(paths::MUTEX_NAME);
    let _mutex = unsafe { CreateMutexW(None, true, PCWSTR(name.as_ptr())) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return; // 이미 실행 중(자동시작 + 수동 실행 등) — 중복 트레이 방지.
    }
    let log = Arc::new(DiagLogger::new(&exe_dir(), true));
    log.log("LogiFlow 시작(트레이)");
    legacy::import_if_first_run(&log);
    if !paths::config_path().exists() {
        // 시험판 한정: 처음 설치(C# 판 설정 없음)도 로그인 자동 시작은 꺼 둔다(C# 판과 두 트레이 동시 자동 시작 방지).
        let first = AppConfig {
            auto_start: false,
            ..Default::default()
        };
        save_config(&first);
        log.log("첫 실행: 기본 설정 저장(시험판 — 자동 시작 꺼짐)");
    }
    let config = load_config();
    log.set_enabled(config.diag_log_enabled);

    wnd::init_dispatcher();
    glow_win::init(config.theme);
    edge_watcher::init(log.clone(), Rc::new(on_edge_trigger));

    let taskbar_created = notify::taskbar_created_msg();
    let handler: Handler = Rc::new(move |h, msg, wp, lp| main_wnd_proc(h, msg, wp, lp, taskbar_created));
    let hwnd = match wnd::create("LogiFlowRs.Tray", "LogiFlow", WS_EX_TOOLWINDOW, WS_OVERLAPPED, (0, 0, 0, 0), None, handler) {
        Some(h) => h,
        None => return,
    };
    let ipc_handler: Handler = Rc::new(|_h, msg, _w, lp| {
        if msg == WM_COPYDATA {
            let (kind, text) = ipc::read(lp);
            // 보낸 쪽을 오래 붙잡지 않게 실제 처리는 나중에.
            wnd::post(move || on_ipc(kind, text));
            return Some(LRESULT(1));
        }
        None
    });
    let _ipc = wnd::message_window(paths::TRAY_IPC_CLASS, ipc_handler);

    let last_results = cache::load(&paths::devices_path());
    T.with(|t| {
        *t.borrow_mut() = Some(Tray {
            log: log.clone(),
            config,
            last_results,
            scanning: false,
            signature: Arc::new(Mutex::new(String::new())),
            last_device_change: 0,
            hwnd,
            ui_hwnd: None,
            ui_child: None,
            exiting: false,
        })
    });

    clipboard::set_events(ClipEvents {
        on_received: Rc::new(|desc: &str| {
            with(|t| notify::balloon(t.hwnd, "LogiFlow", &format!("클립보드 받음: {}", desc), false));
        }),
        on_auth_fail: Rc::new(|| {
            with(|t| {
                notify::balloon(
                    t.hwnd,
                    "LogiFlow 클립보드",
                    "데이터는 받았지만 열지 못했습니다 — 양쪽 암호문구가 다릅니다.",
                    true,
                )
            });
        }),
    });
    if let Some(cfg) = with(|t| t.config.clone()) {
        clipboard::apply(&cfg, &log);
        // 세션 잠금/해제를 워커에 알리는 플래그(ARM 은 잠금 시 데스크톱 이름이 Default).
        workers::set_locked(false);
        unsafe {
            let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        }
        notify::add(hwnd, "LogiFlow");
        apply_autostart(cfg.auto_start);
    }
    reconfigure_watcher();
    update_tray(None);

    // 캐시가 비었으면 첫 스캔, 아니면 캐시 표시 유지.
    wnd::post(|| {
        let sig = scan::signature(&WinHidBackend);
        let empty = with(|t| {
            *t.signature.lock().unwrap() = sig;
            t.last_results.is_empty()
        })
        .unwrap_or(false);
        if empty {
            rescan();
        }
    });

    wnd::run_message_loop();
    log.log("LogiFlow 종료");
}

fn apply_autostart(on: bool) {
    std::thread::spawn(move || autostart::apply(on));
}

fn main_wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, taskbar_created: u32) -> Option<LRESULT> {
    match msg {
        WM_TRAY => {
            let m = (lp.0 & 0xFFFF) as u32;
            if m == WM_LBUTTONDBLCLK {
                show_settings();
            } else if m == WM_RBUTTONUP {
                show_menu();
            }
            Some(LRESULT(0))
        }
        WM_DEVICECHANGE => {
            on_device_changed();
            None
        }
        WM_WTSSESSION_CHANGE => {
            match wp.0 {
                WTS_SESSION_LOCK => workers::set_locked(true),
                WTS_SESSION_UNLOCK => workers::set_locked(false),
                _ => {}
            }
            Some(LRESULT(0))
        }
        WM_CLOSE => Some(LRESULT(0)),
        m if m == taskbar_created && taskbar_created != 0 => {
            // 탐색기 재시작 — 아이콘 다시 등록.
            notify::add(hwnd, "LogiFlow");
            update_tray(None);
            Some(LRESULT(0))
        }
        _ => None,
    }
}

// ---- 상태 조회 ----

fn switchable(t: &Tray) -> Vec<ScanResult> {
    scan::switchable(&t.last_results)
}

fn current_channel(t: &Tray) -> i32 {
    switchable(t).iter().find(|r| r.current_host >= 0).map(|r| r.current_host).unwrap_or(-1)
}

fn max_host_count(t: &Tray) -> i32 {
    switchable(t).iter().map(|r| r.host_count).max().unwrap_or(0).max(0)
}

// ---- 트레이 ----

fn update_tray(status: Option<&str>) {
    with(|t| {
        let cur = current_channel(t);
        let mut title = String::from("LogiFlow");
        if let Some(s) = status {
            title.push_str(" — ");
            title.push_str(s);
        } else if cur >= 0 {
            title.push_str(&format!(" — 채널 {} · {}", cur + 1, t.config.alias_for(cur)));
        }
        // NotifyIcon.Text 최대 63자(C# 은 62자에서 자름).
        let title: String = title.chars().take(62).collect();
        notify::set_tip(t.hwnd, &title);
    });
}

fn show_menu() {
    let built = with(|t| {
        let cur = current_channel(t);
        let max = max_host_count(t);
        let mut items = vec![
            MenuItem::Item {
                id: 9999,
                text: if cur >= 0 {
                    format!("현재 채널: {} · {}", cur + 1, t.config.alias_for(cur))
                } else {
                    "현재 채널: (미탐지)".into()
                },
                enabled: false,
            },
            MenuItem::Separator,
        ];
        let mut any = false;
        for h in 0..max {
            if h == cur {
                continue;
            }
            any = true;
            items.push(MenuItem::Item {
                id: ID_CHANNEL + h as u32,
                text: format!("채널 {} · {}", h + 1, t.config.alias_for(h)),
                enabled: true,
            });
        }
        if !any {
            items.push(MenuItem::Item {
                id: 9998,
                text: "전환할 채널 없음".into(),
                enabled: false,
            });
        }
        items.push(MenuItem::Separator);
        items.push(MenuItem::Item {
            id: ID_SETTINGS,
            text: "설정 열기".into(),
            enabled: true,
        });
        items.push(MenuItem::Item {
            id: ID_RESCAN,
            text: "새로 고침(스캔)".into(),
            enabled: true,
        });
        items.push(MenuItem::Separator);
        items.push(MenuItem::Item {
            id: ID_EXIT,
            text: "종료".into(),
            enabled: true,
        });
        (t.hwnd, items)
    });
    let (hwnd, items) = match built {
        Some(b) => b,
        None => return,
    };
    let mut pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut pt);
    }
    match notify::popup_menu(hwnd, &items, pt.x, pt.y, true) {
        ID_SETTINGS => show_settings(),
        ID_RESCAN => rescan(),
        ID_EXIT => exit_app(),
        id if (ID_CHANNEL..ID_CHANNEL + 16).contains(&id) => quick_switch((id - ID_CHANNEL) as i32),
        _ => {}
    }
}

fn quick_switch(channel: i32) {
    let job = with(|t| {
        t.log.log(&format!("트레이 빠른 전환 → 채널 {}", channel + 1));
        (switchable(t), t.log.clone())
    });
    if let Some((devices, log)) = job {
        std::thread::spawn(move || {
            let sent = switcher::switch_all(&WinHidBackend, &devices, channel, &log);
            log.log(&format!("트레이 전환 전송: {}개", sent));
            wnd::post(|| update_tray(None));
        });
    }
}

fn on_edge_trigger(channel: i32) {
    let job = with(|t| {
        t.log.log(&format!("엣지 트리거 수신 → 채널 {} 전환", channel + 1));
        (switchable(t), t.log.clone())
    });
    if let Some((devices, log)) = job {
        std::thread::spawn(move || {
            let sent = switcher::switch_all(&WinHidBackend, &devices, channel, &log);
            log.log(&format!("엣지 전환 전송: {}개", sent));
            if sent > 0 {
                glow_win::delivered();
            } else {
                glow_win::failed();
            }
            wnd::post(|| update_tray(None));
        });
    }
}

// ---- 스캔 ----

fn rescan() {
    let start = with(|t| {
        if t.scanning {
            return None;
        }
        t.scanning = true;
        Some(t.log.clone())
    })
    .flatten();
    let log = match start {
        Some(l) => l,
        None => return,
    };
    update_tray(Some("스캔 중..."));
    push_state(); // 진단 화면에 즉시 '스캔 중…'
    std::thread::spawn(move || {
        let results = scan::scan(&WinHidBackend, &log);
        let sig = scan::signature(&WinHidBackend);
        wnd::post(move || {
            with(|t| {
                t.last_results = results;
                *t.signature.lock().unwrap() = sig;
                t.scanning = false;
                let _ = cache::save(&paths::devices_path(), &t.last_results);
                // 이 PC 의 호스트(채널)를 기억 — 로그온 전 '당기기' 대상.
                let my = current_channel(t);
                if my >= 0 && t.config.my_host != my {
                    t.config.my_host = my;
                    save_config(&t.config);
                }
            });
            reconfigure_watcher();
            update_tray(None);
            push_state();
        });
    });
}

/// WM_DEVICECHANGE — 디바운스 1.2초 + 안정화 0.6초 뒤 지문이 바뀌었을 때만 재스캔.
fn on_device_changed() {
    let sig = with(|t| {
        let now = tick_ms();
        if now.saturating_sub(t.last_device_change) < 1200 && t.last_device_change != 0 {
            return None;
        }
        t.last_device_change = now;
        Some((t.signature.clone(), t.log.clone()))
    })
    .flatten();
    if let Some((sig, log)) = sig {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(600));
            let now = scan::signature(&WinHidBackend);
            if *sig.lock().unwrap() != now {
                log.log("기기 변화 감지 → 재스캔");
                wnd::post(rescan);
            }
        });
    }
}

// ---- 설정 적용 ----

fn reconfigure_watcher() {
    let st = with(|t| (t.config.clone(), switchable(t).len(), t.log.clone()));
    let (cfg, sw, log) = match st {
        Some(s) => s,
        None => return,
    };
    edge_watcher::stop();
    edge_watcher::configure(cfg.dwell_ms, cfg.drag_guard, &cfg.attachments);
    // B안: 서비스가 켜지면 바탕화면 전환은 서비스 워커가 전담(트레이는 감시 안 함).
    if !cfg.pre_logon && cfg.edge_switch_enabled && edge_watcher::has_attachments() && sw > 0 {
        if legacy::conflict() {
            log.log("기존 LogiFlow(C# 판) 실행 중 — 가장자리 감시 끔(중복 전환 방지)");
        } else {
            edge_watcher::start();
        }
    }
}

fn apply_config(new_cfg: AppConfig) {
    let r = with(|t| {
        let my = t.config.my_host;
        t.config = new_cfg;
        t.config.my_host = my; // 이 PC 채널은 트레이가 스캔으로 정한다.
        save_config(&t.config);
        t.log.set_enabled(t.config.diag_log_enabled);
        (t.config.clone(), t.log.clone())
    });
    let (cfg, log) = match r {
        Some(r) => r,
        None => return,
    };
    apply_autostart(cfg.auto_start);
    // 로그온 전환이 켜져 있으면 머신 공유 엣지 설정을 최신으로(워커가 읽음).
    if cfg.pre_logon {
        write_logon_edges(&cfg);
    }
    glow_win::set_theme(cfg.theme);
    clipboard::apply(&cfg, &log);
    reconfigure_watcher();
    update_tray(None);
    push_state();
}

pub fn write_logon_edges(cfg: &AppConfig) {
    let names = monitors::names(&monitors::all());
    let _ = write_utf8_bom(&paths::logon_edges_path(), &logon_edges::serialize(cfg, &names));
}

pub fn remove_logon_edges() {
    let _ = std::fs::remove_file(paths::logon_edges_path());
}

// ---- 설정 창(별도 프로세스) ----

fn ui_alive(t: &mut Tray) -> bool {
    match t.ui_child.as_mut() {
        Some(c) => matches!(c.try_wait(), Ok(None)),
        None => false,
    }
}

fn show_settings() {
    let act = with(|t| {
        if ui_alive(t) {
            t.ui_hwnd
        } else {
            t.ui_hwnd = None;
            t.ui_child = spawn_self(&["--ui"]);
            None
        }
    })
    .flatten();
    if let Some(h) = act {
        ipc::send(h.get(), ipc::ACTIVATE, "", 1000);
    }
}

fn state_text(t: &Tray) -> String {
    ipc::encode_state(&ipc::State {
        scanning: t.scanning,
        clip_running: clipboard::running(),
        config_plain: t.config.serialize(None),
        devices: cache::serialize(&t.last_results),
    })
}

fn push_state() {
    let s = with(|t| t.ui_hwnd.map(|h| (h, state_text(t)))).flatten();
    if let Some((h, text)) = s {
        ipc::send(h.get(), ipc::STATE, &text, 1000);
    }
}

fn on_ipc(kind: usize, text: String) {
    match kind {
        ipc::HELLO => {
            with(|t| t.ui_hwnd = ipc::parse_hwnd(&text));
            push_state();
        }
        ipc::SET_CONFIG => apply_config(AppConfig::from_plain_text(&text)),
        ipc::RESCAN => rescan(),
        ipc::SWITCH_ONE => {
            let p: Vec<&str> = text.split('\t').collect();
            if p.len() < 3 {
                return;
            }
            let (path, idx, target) = (p[0].to_string(), p[1].parse::<u8>().unwrap_or(0), p[2].parse::<i32>().unwrap_or(-1));
            let job = with(|t| {
                (
                    t.last_results.iter().find(|r| r.path == path && r.device_index == idx).cloned(),
                    t.log.clone(),
                    t.ui_hwnd,
                )
            });
            if let Some((Some(dev), log, ui)) = job {
                std::thread::spawn(move || {
                    let ok = switcher::switch_one(&WinHidBackend, &dev, target, &log);
                    if let Some(h) = ui {
                        ipc::send(h.get(), ipc::SWITCH_RESULT, if ok { "1" } else { "0" }, 2000);
                    }
                });
            } else if let Some((None, _, Some(h))) = job {
                ipc::send(h.get(), ipc::SWITCH_RESULT, "0", 1000);
            }
        }
        ipc::BYE => {
            with(|t| t.ui_hwnd = None);
        }
        _ => {}
    }
}

fn exit_app() {
    let go = with(|t| {
        if t.exiting {
            return None;
        }
        t.exiting = true;
        if let Some(mut c) = t.ui_child.take() {
            let _ = c.kill(); // 우리가 띄운 설정 창(자식 핸들로만 종료)
        }
        Some((t.hwnd, t.log.clone()))
    })
    .flatten();
    if let Some((hwnd, log)) = go {
        unsafe {
            let _ = WTSUnRegisterSessionNotification(hwnd);
        }
        edge_watcher::dispose();
        glow_win::dispose();
        clipboard::stop();
        notify::remove(hwnd);
        log.log("LogiFlow 종료(트레이)");
        wnd::quit();
    }
}

/// 설정 창이 IPC 없이도 쓰는 설정 읽기(트레이가 안 떠 있을 때).
pub fn read_config_direct() -> AppConfig {
    load_config()
}
