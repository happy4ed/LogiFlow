//! 서비스가 투입하는 워커 2종 — C# `DesktopProbe.RunLogonSwitch`(로그온/잠금, 폴링)와
//! `DesktopSwitchWorker`(바탕화면, raw 입력 + glow) 이식. 쓰기는 코어 switcher 한 경로뿐.

use crate::edge_watcher;
use crate::glow_win;
use crate::hid::WinHidBackend;
use crate::monitors;
use crate::paths;
use crate::session::{self, probe_log, NO_SESSION};
use crate::util::{exe_dir, tick_ms};
use crate::wnd;
use logiflow_core::config::{EdgeAttachment, ThemeMode};
use logiflow_core::edge::{EdgeDetector, HandoffEdge, HandoffRect, DEFAULT_BAND_PX};
use logiflow_core::log::{read_lines, DiagLogger};
use logiflow_core::logon_edges::{self, LogonEdgeFile};
use logiflow_core::scan::{self, ScanResult};
use logiflow_core::switcher;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, KillTimer, SetTimer, WM_TIMER};

fn read_edges() -> LogonEdgeFile {
    read_edges_opt().unwrap_or_default()
}

/// 파일이 없거나 비었으면 None(쓰는 중·지워진 순간에 배치를 비워 버리지 않게).
fn read_edges_opt() -> Option<LogonEdgeFile> {
    let lines = read_lines(&paths::logon_edges_path())?;
    if !lines.iter().any(|l| l.trim_start().starts_with("dwellMs=")) {
        return None;
    }
    Some(logon_edges::parse(&lines))
}

/// 바탕화면 워커용 — 머신 공유 파일의 배치를 EdgeAttachment 로.
fn attachments_of(cfg: &LogonEdgeFile) -> Vec<EdgeAttachment> {
    cfg.edges
        .iter()
        .map(|e| EdgeAttachment {
            channel: e.channel,
            monitor_device_name: e.device.clone(),
            edge: HandoffEdge::from_i32(e.edge).unwrap_or(HandoffEdge::Right),
        })
        .collect()
}

fn edges_mtime() -> Option<std::time::SystemTime> {
    std::fs::metadata(paths::logon_edges_path()).and_then(|m| m.modified()).ok()
}

pub fn is_locked() -> bool {
    paths::lock_flag_path().exists()
}

/// 세션 잠금 플래그 — 사용자 세션(트레이)이 잠김 시 만들고 해제 시 지운다. 워커가 읽는다.
pub fn set_locked(locked: bool) {
    let p = paths::lock_flag_path();
    if locked {
        let _ = std::fs::create_dir_all(paths::machine_dir());
        let _ = std::fs::write(&p, session::local_time_hms());
    } else if p.exists() {
        let _ = std::fs::remove_file(&p);
    }
}

fn scan_switchable() -> Vec<ScanResult> {
    scan::switchable(&scan::scan(&WinHidBackend, &DiagLogger::null()))
}

// ===================== 로그온/잠금 화면 워커 =====================

struct Det {
    det: EdgeDetector,
    rect: HandoffRect,
    edge: HandoffEdge,
    channel: i32,
    label: String,
}

/// `--logon-switch` — winlogon 데스크톱에서 보안 화면(잠금/로그온)일 때만 엣지 감지 → setHost. 무한 실행.
pub fn run_logon_switch() {
    // SetThreadDesktop 은 창·훅 없는 스레드에서만 성공 — 새 스레드에서 돈다.
    let t = std::thread::spawn(logon_switch_loop);
    let _ = t.join();
}

fn logon_switch_loop() {
    probe_log("==================================================");
    probe_log(&format!("logon-switch 워커 시작 · 계정={}", session::who()));
    let cfg = read_edges();
    let mut debug = cfg.debug;
    probe_log(&format!(
        "설정 파일 = {} · 디버그={}",
        paths::logon_edges_path().display(),
        if debug { "켜짐" } else { "꺼짐" }
    ));

    let hlog = DiagLogger::new(&exe_dir(), true);
    let initial = scan_switchable();
    probe_log(&format!(
        "엣지 {}개 · 전환가능 기기 {}개 · dwell={}ms",
        cfg.edges.len(),
        initial.len(),
        cfg.dwell_ms
    ));

    // 콜드부팅 로그인 전엔 기기가 자고 있어 스캔이 흔들린다 → 켜진 동안 백그라운드로 주기 스캔해 둔다.
    let shared = Arc::new(Mutex::new(initial));
    let active = Arc::new(AtomicBool::new(false));
    {
        let shared = shared.clone();
        let active = active.clone();
        std::thread::spawn(move || {
            let mut last = 0u64;
            loop {
                std::thread::sleep(Duration::from_millis(200));
                if !active.load(Ordering::SeqCst) {
                    continue; // 사용자 바탕화면에선 스캔 안 함(HID 경합 방지)
                }
                let have = shared.lock().map(|v| v.len()).unwrap_or(0);
                let t = tick_ms();
                // 기기가 있으면 5초마다 갱신, 없으면 1.5초마다 재탐색.
                if if have > 0 { t - last < 5000 } else { t - last < 1500 } {
                    continue;
                }
                last = t;
                let list = scan_switchable();
                if !list.is_empty() {
                    if let Ok(mut v) = shared.lock() {
                        *v = list; // 0 이면 직전 목록 유지(일시적 누락 무시)
                    }
                }
            }
        });
    }
    let mut last_logged = shared.lock().map(|v| v.len()).unwrap_or(0);

    let scr = monitors::all();
    if debug {
        probe_log(&format!("모니터 {}개(워커 기준 좌표):", scr.len()));
        for (i, m) in scr.iter().enumerate() {
            let b = m.bounds;
            probe_log(&format!(
                "  [{}] {} bounds=({},{} {}x{}){}",
                i,
                m.name,
                b.left,
                b.top,
                b.width,
                b.height,
                if m.primary { " 주" } else { "" }
            ));
        }
    }
    let mut dets = build_dets(&cfg, &scr, debug);
    let mut cur_cfg = cfg;

    let original = session::thread_desktop();
    let (mut last_x, mut last_y, mut have_last) = (0, 0, false);
    let (mut last_switch, mut last_debug, mut last_cfg_read) = (0u64, 0u64, 0u64);
    let mut last_name: Option<String> = None;
    // 콘솔이 다른 세션으로 옮겨가면 이 워커는 무효 → 자진 종료(서비스가 새 세션에 재투입).
    let my_session = session::current_session();
    let mut gone_since = 0u64;

    loop {
        std::thread::sleep(Duration::from_millis(50));
        if my_session != NO_SESSION {
            let ac = session::active_console_session();
            if ac != NO_SESSION && ac != my_session {
                let t = tick_ms();
                if gone_since == 0 {
                    gone_since = t;
                } else if t - gone_since > 3000 {
                    probe_log(&format!("콘솔 세션 이탈(내 세션 {} → 활성 {}) — 워커 종료", my_session, ac));
                    return;
                }
            } else {
                gone_since = 0;
            }
        }
        let hin = match session::open_input_desktop() {
            Some(h) => h,
            None => {
                have_last = false;
                continue;
            }
        };
        let name = session::desktop_name(hin);
        let secure = !name.eq_ignore_ascii_case("Default");
        let locked = is_locked();
        // 보안 데스크톱(로그온 화면) 또는 세션 잠김(ARM 은 잠금 때도 Default).
        let is_active = secure || locked;
        active.store(is_active, Ordering::SeqCst);
        let mut p = POINT::default();
        let mut got = false;
        if is_active || debug {
            if session::set_thread_desktop(hin).is_ok() {
                got = unsafe { GetCursorPos(&mut p) }.is_ok();
            }
            if !original.is_invalid() {
                let _ = session::set_thread_desktop(original);
            }
        }
        session::close_desktop(hin);

        let now = tick_ms();
        if now - last_cfg_read > 4000 {
            last_cfg_read = now;
            // 서비스가 켜진 채 배치·대기 시간을 바꿔도 워커를 다시 띄우지 않고 반영(지금 판은 디버그만 다시 읽었음).
            if let Some(nc) = read_edges_opt() {
                if nc.debug != debug {
                    debug = nc.debug;
                    probe_log(&format!("디버그 모드 {} (설정 반영)", if debug { "켜짐" } else { "꺼짐" }));
                }
                if nc.edges != cur_cfg.edges || nc.dwell_ms != cur_cfg.dwell_ms {
                    dets = build_dets(&nc, &monitors::all(), debug);
                    probe_log(&format!("설정 바뀜 → 엣지 {}개 · dwell={}ms 다시 적용", dets.len(), nc.dwell_ms));
                }
                cur_cfg = nc;
            }
        }
        if last_name.as_deref() != Some(name.as_str()) {
            probe_log(&format!(
                "입력 데스크톱={} · 잠김={}{}",
                name,
                if locked { "Y" } else { "N" },
                if is_active {
                    " → 전환 감지 ON"
                } else {
                    " → OFF(사용자 세션 담당)"
                }
            ));
            last_name = Some(name.clone());
        }
        let cur_count = shared.lock().map(|v| v.len()).unwrap_or(0);
        if cur_count != last_logged {
            probe_log(&format!("전환가능 기기 {}개 → {}개 (백그라운드 스캔)", last_logged, cur_count));
            last_logged = cur_count;
        }
        if debug && got && now - last_debug > 400 {
            let mut sb = format!(
                "desktop={} secure={} locked={} active={} cursor=({},{})",
                name,
                if secure { "Y" } else { "N" },
                if locked { "Y" } else { "N" },
                if is_active { "Y" } else { "N" },
                p.x,
                p.y
            );
            for d in &dets {
                let dist = EdgeDetector::distance_from_edge(&d.rect, d.edge, p.x, p.y);
                sb.push_str(&format!(
                    " | {} dist={} band={} dwell={:.2}",
                    d.label,
                    dist,
                    if dist <= DEFAULT_BAND_PX { "Y" } else { "N" },
                    d.det.progress(now as i64)
                ));
            }
            probe_log(&sb);
            last_debug = now;
        }
        if !is_active || !got {
            have_last = false;
            for d in &mut dets {
                d.det.reset(true);
            }
            continue;
        }
        let (dx, dy) = if have_last { (p.x - last_x, p.y - last_y) } else { (0, 0) };
        last_x = p.x;
        last_y = p.y;
        have_last = true;
        if now - last_switch < 1200 {
            continue;
        }
        let mut fired: Option<(i32, String)> = None;
        for d in &mut dets {
            let push = EdgeDetector::outward_push(d.edge, dx, dy);
            if d.det.feed(&d.rect, d.edge, p.x, p.y, push, now as i64, false, false) {
                fired = Some((d.channel, d.label.clone()));
                break;
            }
        }
        if let Some((channel, label)) = fired {
            probe_log(&format!("[로그온 전환] {} → 채널 {} · cursor=({},{})", label, channel + 1, p.x, p.y));
            // 백그라운드 스캐너의 최신 목록에서 즉시 전송.
            let sw = shared.lock().map(|v| v.clone()).unwrap_or_default();
            if sw.is_empty() {
                probe_log("  전환가능 기기 0개 — 마우스·키보드가 아직 자고 있음(한 번 움직여/눌러 깨우면 다음 밀기에 전환).");
            }
            let ok = switcher::switch_all_verbose(&WinHidBackend, &sw, channel, &hlog, &mut |s| probe_log(s), 3);
            probe_log(&format!("  setHost 전환 {}/{}개", ok, sw.len()));
            last_switch = now;
            for o in &mut dets {
                o.det.reset(true);
            }
        }
    }
}

/// 로그인 화면 워커의 가장자리 판정기 — logon-edges 의 배치·대기 시간으로 만든다(시작 때 + 설정이 바뀔 때마다).
fn build_dets(cfg: &LogonEdgeFile, scr: &[monitors::Monitor], debug: bool) -> Vec<Det> {
    let mut dets: Vec<Det> = Vec::new();
    for e in &cfg.edges {
        let s = scr
            .iter()
            .find(|m| m.name == e.device)
            .or_else(|| if e.index >= 0 { scr.get(e.index as usize) } else { None });
        let s = match s {
            Some(s) => s,
            None => {
                probe_log(&format!("엣지 매칭 실패(모니터 없음): {} idx={}", e.device, e.index));
                continue;
            }
        };
        let edge = HandoffEdge::from_i32(e.edge).unwrap_or(HandoffEdge::Right);
        // 폴링은 화면 끝에 막힌 커서의 '바깥 밀기'를 못 읽는다 → 밀기 요건 0 = 변에서 dwell 채우면 전환.
        let d = Det {
            det: EdgeDetector::new(cfg.dwell_ms, 0),
            rect: s.bounds,
            edge,
            channel: e.channel,
            label: format!("{} {}", s.name, edge.name()),
        };
        if debug {
            let b = s.bounds;
            probe_log(&format!(
                "엣지 등록: {} mon=({},{} {}x{}) → 채널 {}",
                d.label,
                b.left,
                b.top,
                b.width,
                b.height,
                d.channel + 1
            ));
        }
        dets.push(d);
    }
    if dets.is_empty() {
        probe_log("※ 적용할 엣지 0개 — [전환 관리]에서 채널 박스를 변에 붙였는지 확인하세요.");
    }
    dets
}

// ===================== 바탕화면 워커 =====================

/// `--desktop-switch` — 서비스가 winsta0\default 에 띄움. 트레이와 같은 raw 판정 + glow.
pub fn run_desktop_switch() {
    let log = Arc::new(DiagLogger::new(&exe_dir(), true));
    let hlog = Arc::new(DiagLogger::new(&exe_dir(), true));
    let say = {
        let log = log.clone();
        move |s: &str| log.log(s)
    };
    say("==================================================");
    say("desktop-switch(B) 워커 시작 — Default 데스크탑 raw 엣지 전환");
    // 수정 시각을 먼저 잡는다 — 읽은 뒤 시작 준비(스캔) 중에 파일이 바뀌면 첫 타이머에서 다시 적용되게.
    let started_mtime = edges_mtime();
    let cfg = read_edges();
    let attachments = attachments_of(&cfg);
    say(&format!("엣지 {}개 · dwell={}ms", attachments.len(), cfg.dwell_ms));

    // setHost 는 별도 스레드(UI 스레드가 막히면 glow 가 멈춤). 진행 중 표시·마지막 전환 시각은 스캐너도 본다.
    let switching = Arc::new(AtomicBool::new(false));
    let last_switch = Arc::new(AtomicI64::new(0));
    let box_ = Arc::new(Mutex::new(scan_switchable()));
    say(&format!("전환가능 기기 {}개", box_.lock().map(|v| v.len()).unwrap_or(0)));
    {
        let box_ = box_.clone();
        let switching = switching.clone();
        let last_switch = last_switch.clone();
        std::thread::spawn(move || {
            let mut last = tick_ms();
            loop {
                std::thread::sleep(Duration::from_millis(300));
                let have = box_.lock().map(|v| v.len()).unwrap_or(0);
                let t = tick_ms();
                // 기기가 있으면 60초마다만, 전환 중·전환 뒤 10초는 건너뜀. 없으면 1.5초마다 재탐색.
                if if have > 0 { t - last < 60000 } else { t - last < 1500 } {
                    continue;
                }
                if switching.load(Ordering::SeqCst) {
                    continue;
                }
                if (t as i64) - last_switch.load(Ordering::SeqCst) < 10000 {
                    continue;
                }
                last = t;
                let list = scan_switchable();
                if !list.is_empty() {
                    if let Ok(mut v) = box_.lock() {
                        *v = list;
                    }
                }
            }
        });
    }

    wnd::init_dispatcher();
    glow_win::init(ThemeMode::System);
    let on_trigger: Rc<dyn Fn(i32)> = {
        let box_ = box_.clone();
        let switching = switching.clone();
        let last_switch = last_switch.clone();
        let log = log.clone();
        let hlog = hlog.clone();
        Rc::new(move |channel: i32| {
            let devs = box_.lock().map(|v| v.clone()).unwrap_or_default();
            log.log(&format!("[바탕화면 전환] → 채널 {} · 기기 {}개", channel + 1, devs.len()));
            if devs.is_empty() {
                log.log("  전환가능 기기 0개 — 건너뜀");
                glow_win::failed();
                return;
            }
            if switching.swap(true, Ordering::SeqCst) {
                log.log("  이전 전환 진행 중 — 건너뜀");
                glow_win::failed();
                return;
            }
            last_switch.store(tick_ms() as i64, Ordering::SeqCst);
            let (switching, last_switch, log, hlog) = (switching.clone(), last_switch.clone(), log.clone(), hlog.clone());
            std::thread::spawn(move || {
                let ok = switcher::switch_all_burst(&WinHidBackend, &devs, channel, &hlog, &mut |s| log.log(s), 3, Some(&glow_win::delivered));
                log.log(&format!("  setHost 전환 {}/{}개", ok, devs.len()));
                last_switch.store(tick_ms() as i64, Ordering::SeqCst);
                switching.store(false, Ordering::SeqCst);
                if ok == 0 {
                    // 하나도 못 넘겼으면 glow 를 흐리고, 계속 밀어도 재시도되게 재무장.
                    glow_win::failed();
                    wnd::post(edge_watcher::rearm);
                }
            });
        })
    };
    edge_watcher::init(log.clone(), on_trigger);
    edge_watcher::configure(cfg.dwell_ms, cfg.drag_guard, &attachments);
    edge_watcher::start();

    // self-exit: 콘솔 세션이 바뀌면 3초 뒤 종료 → supervisor 가 교체.
    let my_session = session::current_session();
    let off = std::cell::Cell::new(0);
    // 서비스가 켜진 채 「끌기 중 전환 막기」·배치·머무는 시간을 바꾸면 1초 안에 판정기에 다시 적용
    // (트레이가 logon-edges 를 새로 쓰면 수정 시각이 바뀐다). 지금 판은 워커가 다시 뜰 때까지 옛 값이었다.
    let applied = std::cell::RefCell::new((started_mtime, cfg));
    let timer_handler: wnd::Handler = Rc::new(move |_h, msg, _w, _l| {
        if msg == WM_TIMER {
            let m = edges_mtime();
            if m != applied.borrow().0 {
                if let Some(nc) = read_edges_opt() {
                    let mut a = applied.borrow_mut();
                    if nc.edges != a.1.edges || nc.dwell_ms != a.1.dwell_ms || nc.drag_guard != a.1.drag_guard {
                        edge_watcher::configure(nc.dwell_ms, nc.drag_guard, &attachments_of(&nc));
                        probe_log(&format!(
                            "desktop-switch 워커: 설정 바뀜 → 엣지 {}개 · dwell={}ms · 끌기 중 막기={}",
                            nc.edges.len(),
                            nc.dwell_ms,
                            if nc.drag_guard { "켜짐" } else { "꺼짐" }
                        ));
                    }
                    *a = (m, nc);
                }
            }
            let cur = session::active_console_session();
            if cur != NO_SESSION && cur != my_session {
                off.set(off.get() + 1);
                if off.get() >= 3 {
                    probe_log("desktop-switch 워커: 콘솔 세션 변경 → 종료");
                    wnd::quit();
                }
            } else {
                off.set(0);
            }
        }
        None
    });
    let timer_wnd = wnd::message_window("LogiFlowRs.WorkerTimer", timer_handler);
    if let Some(h) = timer_wnd {
        unsafe {
            SetTimer(Some(h), 1, 1000, None);
        }
    }
    wnd::run_message_loop();
    if let Some(h) = timer_wnd {
        unsafe {
            let _ = KillTimer(Some(h), 1);
        }
    }
    edge_watcher::dispose();
    glow_win::dispose();
}
