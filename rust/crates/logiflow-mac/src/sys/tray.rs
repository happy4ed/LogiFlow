//! 메뉴 막대 상주 앱 — Windows `tray` 의 Mac 판. 설정·스캔·가장자리 감시·전환·기기 캐시·클립보드의 단일 주인.
//! 설정 창은 `--ui` 별도 프로세스(열 때만)로 Unix 소켓 IPC 로 붙는다(Windows 와 같은 종류 번호·상태 묶음).
//!
//! 메뉴 막대 메뉴는 열 때마다 다시 만든다(Windows 트레이 오른쪽 클릭 메뉴와 같은 항목):
//! 현재 채널 · 「… 모두 전환」 · LogiFlow 창 열기 · 기기 다시 찾기 · 종료.

use super::autostart;
use super::clipboard::{self, ClipEvents};
use super::edge_watcher;
use super::ffi::{cf_bool, cf_bytes, cfstr};
use super::glow_mac;
use super::hid::MacHidBackend;
use super::ipc;
use super::keychain;
use super::mainq;
use super::monitors;
use super::notify::{self, menu_item};
use super::paths;
use super::util::{spawn_self, tick_ms};
use super::wnd::Hwnd;
use logiflow_core::cache;
use logiflow_core::config::AppConfig;
use logiflow_core::log::DiagLogger;
use logiflow_core::scan::{self, ScanResult};
use logiflow_core::switcher;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::{NSData, NSObject, NSObjectProtocol, NSSize, NSString};
use std::cell::RefCell;
use std::process::Child;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const ID_SETTINGS: isize = 1;
const ID_RESCAN: isize = 2;
const ID_EXIT: isize = 3;
const ID_CHANNEL: isize = 1000;

struct Tray {
    log: Arc<DiagLogger>,
    config: AppConfig,
    last_results: Vec<ScanResult>,
    scanning: bool,
    signature: Arc<Mutex<String>>,
    ui_pid: Option<Hwnd>,
    ui_child: Option<Child>,
    exiting: bool,
    item: Retained<NSStatusItem>,
    _menu_target: Retained<TrayMenu>,
    _delegate: Retained<AppDelegate>,
}

thread_local! {
    static T: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Tray) -> R) -> Option<R> {
    T.with(|t| t.try_borrow_mut().ok().and_then(|mut b| b.as_mut().map(f)))
}

// ---- 설정 파일 ----

/// 설정 저장 — 암호문구는 키체인에(파일에는 `clipSecretDpapi=keychain` 표시만). 키체인이 실패하면 파일에
/// 암호문구를 쓰지 않는다(평문으로 대신 쓰지 않음 · 설정 창에 주의 글). 비운 암호문구는 키체인에서도 지운다.
fn save_config(cfg: &AppConfig) {
    paths::ensure_support_dir();
    let path = paths::config_path();
    if cfg.clip_secret.is_empty() {
        keychain::delete();
    }
    let _ = keychain::with_codec(|c| cfg.save(&path, Some(c)));
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
}

fn load_config() -> AppConfig {
    keychain::with_codec(|c| AppConfig::load(&paths::config_path(), Some(c)))
}

/// 0.16.0 Mac 판은 암호문구를 config.txt 에 평문(`clipSecret=…`)으로 두었다 — 키체인으로 옮기고, 저장·되읽기가
/// 맞으면 파일을 평문 없이 다시 쓴다. 키체인이 실패하면 파일은 그대로 둔다(암호문구를 잃지 않게, 다음 실행에 다시 시도).
fn migrate_plain_secret(cfg: &AppConfig, log: &DiagLogger) {
    let lines = logiflow_core::log::read_lines(&paths::config_path()).unwrap_or_default();
    let plain = lines.iter().any(|l| {
        l.trim_start_matches('\u{feff}')
            .strip_prefix("clipSecret=")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    });
    if !plain || cfg.clip_secret.is_empty() {
        return;
    }
    match keychain::store(&cfg.clip_secret) {
        Ok(()) => {
            save_config(cfg);
            let still = logiflow_core::log::read_lines(&paths::config_path()).unwrap_or_default().iter().any(|l| {
                l.trim_start_matches('\u{feff}')
                    .strip_prefix("clipSecret=")
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
            });
            log.log(if still {
                "암호문구 키체인 옮기기: 키체인 저장은 됐으나 파일 평문이 남음"
            } else {
                "암호문구를 키체인으로 옮김(설정 파일의 평문 지움)"
            });
        }
        Err(e) => log.error(&format!("암호문구 키체인 옮기기 실패 — 파일 그대로 둠: {}", e)),
    }
}

/// 설정 창이 상주 앱 없이도 쓰는 설정 읽기(암호문구는 읽지 않는다 — 상주 앱 상태로 받는다).
pub fn read_config_direct() -> AppConfig {
    AppConfig::load(&paths::config_path(), None)
}

/// Windows 의 로그인 전 실행 배치 파일(Mac 에는 없음).
pub fn write_logon_edges(_cfg: &AppConfig) {}
pub fn remove_logon_edges() {}

/// Swift 판 UserDefaults 값 하나(없으면 null 아닌 Cf(null)).
fn swift_value(key: &str) -> super::ffi::Cf {
    let k = cfstr(key);
    let app = cfstr(paths::SWIFT_DOMAIN);
    super::ffi::Cf(unsafe { core_foundation_sys::preferences::CFPreferencesCopyAppValue(k.0 as _, app.0 as _) })
}

/// 첫 실행 — 기본 설정 + Swift 판(0.15.0) 설정 이어받기(가장자리 자동 전환 켜짐 · 변 배치).
fn first_run(log: &DiagLogger) {
    let mut cfg = AppConfig::default();
    let enabled = swift_value("edgeEnabled");
    let bindings = swift_value("edgeBindings");
    let on = cf_bool(enabled.0);
    let json = cf_bytes(bindings.0);
    if crate::migrate::apply(&mut cfg, on, json.as_deref()) {
        log.log(&format!(
            "첫 실행: Swift 판 설정 이어받음(가장자리 자동 전환 {} · 배치 {}개)",
            if cfg.edge_switch_enabled { "켜짐" } else { "꺼짐" },
            cfg.attachments.len()
        ));
    } else {
        log.log("첫 실행: 기본 설정 저장(로그인 자동 시작 켜짐)");
    }
    save_config(&cfg);
}

/// 상주 앱 하나만 — 잠금 파일. 이미 있으면 None.
fn single_instance() -> Option<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    paths::ensure_support_dir();
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths::agent_lock())
        .ok()?;
    let r = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if r == 0 {
        Some(f)
    } else {
        None
    }
}

// ---- 메뉴 막대 ----

pub struct MenuIvars {}

define_class!(
    // SAFETY: NSObject 는 하위 클래스 조건이 없고, 이 클래스는 Drop 을 구현하지 않는다.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LogiFlowRsTrayMenu"]
    #[ivars = MenuIvars]
    struct TrayMenu;

    impl TrayMenu {
        #[unsafe(method(pick:))]
        fn pick(&self, sender: &NSMenuItem) {
            on_menu(sender.tag());
        }
    }

    unsafe impl NSObjectProtocol for TrayMenu {}

    unsafe impl NSMenuDelegate for TrayMenu {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            build_menu(menu, self);
        }
    }
);

pub struct DelegateIvars {}

define_class!(
    // SAFETY: NSObject 는 하위 클래스 조건이 없고, 이 클래스는 Drop 을 구현하지 않는다.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LogiFlowRsAppDelegate"]
    #[ivars = DelegateIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        /// Finder·Dock 에서 앱을 다시 열면(이미 떠 있을 때) 설정 창을 연다.
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_reopen(&self, _app: &NSApplication, _flag: bool) -> bool {
            mainq::post(show_settings);
            true
        }
    }
);

fn tray_icon() -> Option<Retained<NSImage>> {
    // 앱 아이콘(「두 화면 흐름」 마크) 64px PNG 를 템플릿(단색)으로 — 메뉴 막대 색을 따른다.
    let ico: &[u8] = include_bytes!("../../../../../LogiFlow/Resources/logiflow.ico");
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    for i in 0..count {
        let e = ico.get(6 + 16 * i..22 + 16 * i)?;
        if e[0] == 64 {
            let size = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
            let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
            let png = ico.get(off..off + size)?;
            let data = NSData::with_bytes(png);
            let img = NSImage::initWithData(NSImage::alloc(), &data)?;
            img.setSize(NSSize::new(18.0, 18.0));
            img.setTemplate(true);
            return Some(img);
        }
    }
    None
}

fn switchable(t: &Tray) -> Vec<ScanResult> {
    scan::switchable(&t.last_results)
}

fn current_channel(t: &Tray) -> i32 {
    switchable(t).iter().find(|r| r.current_host >= 0).map(|r| r.current_host).unwrap_or(-1)
}

fn max_host_count(t: &Tray) -> i32 {
    switchable(t).iter().map(|r| r.host_count).max().unwrap_or(0).max(0)
}

fn build_menu(menu: &NSMenu, target: &TrayMenu) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    menu.removeAllItems();
    menu.setAutoenablesItems(false);
    let tgt: &AnyObject = target;
    let lines = with(|t| {
        let cur = current_channel(t);
        let max = max_host_count(t);
        let mut v: Vec<Option<(String, isize, bool)>> = vec![
            Some((
                if cur >= 0 {
                    format!("현재 채널: {}", t.config.channel_label(cur))
                } else {
                    "현재 채널: (미탐지)".into()
                },
                9999,
                false,
            )),
            None,
        ];
        let mut any = false;
        for h in 0..max {
            if h == cur {
                continue;
            }
            any = true;
            v.push(Some((format!("{} 모두 전환", t.config.channel_label_to(h)), ID_CHANNEL + h as isize, true)));
        }
        if !any {
            v.push(Some(("전환할 채널 없음".into(), 9998, false)));
        }
        v.push(None);
        v.push(Some(("LogiFlow 창 열기".into(), ID_SETTINGS, true)));
        v.push(Some(("기기 다시 찾기".into(), ID_RESCAN, true)));
        v.push(None);
        v.push(Some(("종료".into(), ID_EXIT, true)));
        v
    })
    .unwrap_or_default();
    for l in lines {
        match l {
            None => menu.addItem(&NSMenuItem::separatorItem(mtm)),
            Some((text, tag, enabled)) => menu.addItem(&menu_item(mtm, &text, tag, enabled, Some(tgt))),
        }
    }
}

fn on_menu(tag: isize) {
    match tag {
        ID_SETTINGS => show_settings(),
        ID_RESCAN => rescan(),
        ID_EXIT => exit_app(),
        id if (ID_CHANNEL..ID_CHANNEL + 16).contains(&id) => switch_all_to((id - ID_CHANNEL) as i32, None),
        _ => {}
    }
}

fn update_tray(status: Option<&str>) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    with(|t| {
        let cur = current_channel(t);
        let mut title = String::from("LogiFlow");
        if let Some(s) = status {
            title.push_str(" — ");
            title.push_str(s);
        } else if cur >= 0 {
            title.push_str(&format!(" — {}", t.config.channel_label(cur)));
        }
        if let Some(b) = t.item.button(mtm) {
            b.setToolTip(Some(&NSString::from_str(&super::dialog::localize(&title))));
        }
    });
}

// ---- 진입점 ----

/// 상주 앱 진입점. show_window = 사람이 앱을 열었을 때(로그인 자동 시작이 아닐 때) 설정 창도 띄운다.
pub fn run(show_window: bool) {
    let Some(_lock) = single_instance() else {
        // 이미 떠 있음 — 그쪽에 창을 열라고만 하고 끝낸다.
        ipc::send(ipc::AGENT, ipc::SHOW, "", 2000);
        return;
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let log = Arc::new(DiagLogger::new(&paths::support_dir(), true));
    log.log("LogiFlow 시작(메뉴 막대)");
    if !paths::config_path().exists() {
        first_run(&log);
    }
    let config = load_config();
    log.set_enabled(config.diag_log_enabled);
    migrate_plain_secret(&config, &log);
    if let Some(n) = keychain::note() {
        log.error(&format!("암호문구 키체인: {}", n));
    }

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let delegate: Retained<AppDelegate> = {
        let this = AppDelegate::alloc(mtm).set_ivars(DelegateIvars {});
        unsafe { msg_send![super(this), init] }
    };
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    glow_mac::init(config.theme);
    edge_watcher::init(log.clone(), Rc::new(on_edge_trigger));

    let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(b) = item.button(mtm) {
        match tray_icon() {
            Some(img) => b.setImage(Some(&img)),
            None => b.setTitle(&NSString::from_str("LogiFlow")),
        }
    }
    let menu_target: Retained<TrayMenu> = {
        let this = TrayMenu::alloc(mtm).set_ivars(MenuIvars {});
        unsafe { msg_send![super(this), init] }
    };
    let menu = NSMenu::new(mtm);
    menu.setDelegate(Some(ProtocolObject::from_ref(&*menu_target)));
    item.setMenu(Some(&menu));

    let last_results = cache::load(&paths::devices_path());
    T.with(|t| {
        *t.borrow_mut() = Some(Tray {
            log: log.clone(),
            config,
            last_results,
            scanning: false,
            signature: Arc::new(Mutex::new(String::new())),
            ui_pid: None,
            ui_child: None,
            exiting: false,
            item,
            _menu_target: menu_target,
            _delegate: delegate,
        })
    });

    // 설정 창 → 상주 앱 IPC(받은 것은 메인 스레드에서 처리).
    if !ipc::listen(paths::agent_socket(), |kind, text| mainq::post(move || on_ipc(kind, text))) {
        log.error("IPC 소켓을 열지 못함 — 설정 창과 연결 안 됨");
    }

    clipboard::set_events(ClipEvents {
        on_received: Rc::new(|desc: &str| notify::balloon("LogiFlow", &format!("클립보드 받음: {}", desc))),
        on_auth_fail: Rc::new(|| notify::balloon("LogiFlow 클립보드", "데이터는 받았지만 열지 못했습니다 — 양쪽 암호문구가 다릅니다.")),
    });
    if let Some(cfg) = with(|t| t.config.clone()) {
        clipboard::apply(&cfg, &log);
        apply_autostart(cfg.auto_start);
    }
    reconfigure_watcher();
    update_tray(None);
    warn_if_swift_running(&log);

    // 캐시가 비었으면 첫 스캔, 아니면 캐시 표시 유지.
    mainq::post(|| {
        let sig = scan::signature(&MacHidBackend);
        let empty = with(|t| {
            *t.signature.lock().unwrap() = sig;
            t.last_results.is_empty()
        })
        .unwrap_or(false);
        if empty {
            rescan();
        }
    });
    start_change_watch();
    if show_window {
        mainq::post(show_settings);
    }
    app.run();
    log.log("LogiFlow 종료");
}

/// 기기·화면 변화 감지(2초마다, HID++ 입출력 없이 목록만) — Windows WM_DEVICECHANGE · 화면 변경 대응.
fn start_change_watch() {
    let Some((sig, log)) = with(|t| (t.signature.clone(), t.log.clone())) else {
        return;
    };
    let screens = |v: Vec<monitors::Monitor>| v.iter().map(|m| format!("{}:{:?}", m.name, m.bounds)).collect::<Vec<_>>().join("|");
    let _ = std::thread::Builder::new().name("LogiFlow.ChangeWatch".into()).spawn(move || {
        let mut last_screens = screens(monitors::all());
        loop {
            std::thread::sleep(std::time::Duration::from_millis(2000));
            let now = scan::signature(&MacHidBackend);
            let known = sig.lock().map(|s| s.clone()).unwrap_or_default();
            if !known.is_empty() && known != now {
                log.log("기기 변화 감지 → 재스캔");
                if let Ok(mut s) = sig.lock() {
                    *s = now; // 같은 변화로 두 번 다시 찾지 않게(스캔이 끝나면 다시 맞춘다)
                }
                mainq::post(rescan);
            }
            let sc = screens(monitors::all());
            if sc != last_screens {
                last_screens = sc;
                log.log("화면 배치 변화 → 가장자리 감시 다시 구성");
                mainq::post(reconfigure_watcher);
            }
        }
    });
}

/// Swift 판(0.15.0, 번들 id com.happy4ed.logiflow)이 함께 돌면 가장자리 전환이 두 번 일어날 수 있다 — 한 번 알린다.
fn warn_if_swift_running(log: &DiagLogger) {
    let list = objc2_app_kit::NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(paths::SWIFT_DOMAIN));
    if list.count() > 0 {
        log.log("Swift 판 LogiFlow 가 실행 중 — 종료 안내");
        notify::balloon(
            "LogiFlow",
            "이전 판 LogiFlow 도 실행 중입니다. 가장자리 전환이 두 번 일어날 수 있으니 이전 판을 종료하세요(메뉴 막대 → 종료).",
        );
    }
}

fn apply_autostart(on: bool) {
    std::thread::spawn(move || autostart::apply(on));
}

// ---- 전환 ----

/// 전환 가능한 기기 전부를 한 채널로(메뉴 · 설정 창 「모두 전환」). ui = 결과를 돌려줄 설정 창.
fn switch_all_to(channel: i32, ui: Option<Hwnd>) {
    let job = with(|t| {
        t.log.log(&format!("모두 전환 → 채널 {}", channel + 1));
        (switchable(t), t.log.clone())
    });
    if let Some((devices, log)) = job {
        std::thread::spawn(move || {
            let sent = switcher::switch_all(&MacHidBackend, &devices, channel, &log);
            log.log(&format!("모두 전환 전송: {}개", sent));
            if let Some(h) = ui {
                ipc::send(h.get(), ipc::SWITCH_RESULT, if sent > 0 { "1" } else { "0" }, 2000);
            }
            mainq::post(|| update_tray(None));
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
            let sent = switcher::switch_all(&MacHidBackend, &devices, channel, &log);
            log.log(&format!("엣지 전환 전송: {}개", sent));
            if sent > 0 {
                glow_mac::delivered();
            } else {
                glow_mac::failed();
                mainq::post(edge_watcher::rearm);
            }
            mainq::post(|| update_tray(None));
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
    let Some(log) = start else {
        return;
    };
    update_tray(Some("기기 찾는 중…"));
    push_state();
    std::thread::spawn(move || {
        let results = scan::scan(&MacHidBackend, &log);
        let sig = scan::signature(&MacHidBackend);
        mainq::post(move || {
            with(|t| {
                t.last_results = results;
                *t.signature.lock().unwrap() = sig;
                t.scanning = false;
                let _ = cache::save(&paths::devices_path(), &t.last_results);
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

// ---- 설정 적용 ----

fn reconfigure_watcher() {
    let Some((cfg, sw)) = with(|t| (t.config.clone(), switchable(t).len())) else {
        return;
    };
    edge_watcher::stop();
    edge_watcher::configure(cfg.dwell_ms, cfg.drag_guard, &cfg.attachments);
    if cfg.tray_should_watch(edge_watcher::has_attachments(), sw) {
        edge_watcher::start();
    }
}

fn apply_config(new_cfg: AppConfig) {
    let r = with(|t| {
        let my = t.config.my_host;
        t.config = new_cfg;
        t.config.my_host = my; // 이 Mac 채널은 상주 앱이 스캔으로 정한다.
        t.config.pre_logon = false; // Mac 에는 없다(켜져 있으면 가장자리 감시가 꺼진다).
        save_config(&t.config);
        t.log.set_enabled(t.config.diag_log_enabled);
        (t.config.clone(), t.log.clone())
    });
    let Some((cfg, log)) = r else {
        return;
    };
    if let Some(n) = keychain::note() {
        log.error(&format!("암호문구 키체인: {}", n));
        notify::balloon("LogiFlow", &n);
    }
    apply_autostart(cfg.auto_start);
    glow_mac::set_theme(cfg.theme);
    clipboard::apply(&cfg, &log);
    reconfigure_watcher();
    update_tray(None);
    push_state();
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
            t.ui_pid
        } else {
            t.ui_pid = None;
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
        secret_note: keychain::note().unwrap_or_default(),
    })
}

fn push_state() {
    let s = with(|t| t.ui_pid.map(|h| (h, state_text(t)))).flatten();
    if let Some((h, text)) = s {
        ipc::send(h.get(), ipc::STATE, &text, 1000);
    }
}

fn on_ipc(kind: usize, text: String) {
    match kind {
        ipc::HELLO => {
            with(|t| t.ui_pid = ipc::parse_hwnd(&text));
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
                    // 전환 가능 기기(0x1814 + 호스트 수 확인)만 — 호스트 수 미확인(-1)이면 거부.
                    scan::switchable(&t.last_results).into_iter().find(|r| r.path == path && r.device_index == idx),
                    t.log.clone(),
                    t.ui_pid,
                )
            });
            if let Some((Some(dev), log, ui)) = job {
                std::thread::spawn(move || {
                    let ok = switcher::switch_one(&MacHidBackend, &dev, target, &log);
                    if let Some(h) = ui {
                        ipc::send(h.get(), ipc::SWITCH_RESULT, if ok { "1" } else { "0" }, 2000);
                    }
                });
            } else if let Some((None, _, Some(h))) = job {
                ipc::send(h.get(), ipc::SWITCH_RESULT, "0", 1000);
            }
        }
        ipc::SWITCH_ALL => {
            if let Ok(target) = text.trim().parse::<i32>() {
                let ui = with(|t| t.ui_pid).flatten();
                switch_all_to(target, ui);
            }
        }
        ipc::BYE => {
            with(|t| t.ui_pid = None);
        }
        ipc::SHOW => show_settings(),
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
        Some(t.log.clone())
    })
    .flatten();
    let Some(log) = go else {
        return;
    };
    edge_watcher::dispose();
    glow_mac::dispose();
    clipboard::stop();
    let _ = std::fs::remove_file(paths::agent_socket());
    log.log("LogiFlow 종료(메뉴 막대)");
    let _ = tick_ms();
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).terminate(None);
    }
}
