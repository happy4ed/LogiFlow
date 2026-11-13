//! 로그온 화면 접근 테스트(실험·읽기 전용) — C# `DesktopProbe.RunProbe`·`RunLauncher`·`ProbeTask` 이식.
//! HID 열거·0x1814 조회만 하고 setHost 는 보내지 않는다.

use crate::hid::WinHidBackend;
use crate::paths::{self, PROBE_TASK};
use crate::session::{self, probe_log as append};
use crate::util::{exe_path, run_elevated_self, run_hidden};
use logiflow_core::log::DiagLogger;
use logiflow_core::scan;
use std::time::Duration;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// `--desktop-probe=N` — N초 동안 입력 데스크톱 커서 읽기를 시도·기록.
pub fn run_probe(seconds: i32) {
    let seconds = seconds.clamp(5, 600);
    append("==================================================");
    append(&format!(
        "desktop-probe 시작 · 실행 계정={} · {}초 · 지금부터 잠금(Win+L) 또는 로그인 화면에서 마우스를 움직여 보세요",
        session::who(),
        seconds
    ));
    let t = std::thread::spawn(move || probe_loop(seconds));
    let _ = t.join();
}

fn probe_loop(seconds: i32) {
    // 이 컨텍스트(SYSTEM·winlogon 데스크톱)에서 HID 가 읽히는지 1회(읽기 전용).
    let results = scan::scan(&WinHidBackend, &DiagLogger::null());
    let sw = results.iter().filter(|r| r.change_host_supported).count();
    append(&format!("--- HID 접근 테스트: 기기 {}개 · 전환가능(0x1814) {}개 ---", results.len(), sw));
    for r in &results {
        append(&format!(
            "  {} [{}] 0x1814={} 현재채널={} {}",
            r.device_name,
            r.vid_pid,
            if r.change_host_supported {
                format!("0x{:02X}", r.change_host_index)
            } else {
                "미지원".into()
            },
            if r.current_host >= 0 { (r.current_host + 1).to_string() } else { "?".into() },
            if r.note.is_empty() { String::new() } else { format!("({})", r.note) }
        ));
    }
    append(if sw > 0 {
        "→ 이 컨텍스트에서 HID 읽기 OK. setHost(전환)도 같은 경로라 로그온 화면 전환 구현 가능."
    } else {
        "→ 이 컨텍스트에서 전환 가능 기기를 못 읽음 — 여기가 ARM/잠금화면 전환 실패의 원인 후보."
    });

    let original = session::thread_desktop();
    let end = crate::util::tick_ms() + seconds as u64 * 1000;
    let mut last_name: Option<String> = None;
    let mut tick = 0;
    while crate::util::tick_ms() < end {
        tick += 1;
        let hin = match session::open_input_desktop() {
            Some(h) => h,
            None => {
                append(&format!("OpenInputDesktop 실패 (입력 데스크톱 접근 거부) err={}", session::last_error()));
                std::thread::sleep(Duration::from_millis(700));
                continue;
            }
        };
        let name = session::desktop_name(hin);
        let set = session::set_thread_desktop(hin);
        let set_err = set.err().unwrap_or(0);
        let mut p = POINT::default();
        let (mut got, mut cur_err) = (false, 0);
        if set_err == 0 {
            got = unsafe { GetCursorPos(&mut p) }.is_ok();
            if !got {
                cur_err = session::last_error();
            }
        }
        if last_name.as_deref() != Some(name.as_str()) {
            append(&format!(">>> 현재 입력 데스크톱 = \"{}\"  (Winlogon = 로그온/잠금 보안 화면)", name));
            last_name = Some(name.clone());
        }
        if tick % 3 == 0 || set_err != 0 || !got {
            append(&format!(
                "desktop=\"{}\" setThreadDesktop={}(err{}) GetCursorPos={}(err{})",
                name,
                if set_err == 0 { "성공" } else { "실패" },
                set_err,
                if got { format!("({},{})", p.x, p.y) } else { "실패".into() },
                cur_err
            ));
        }
        if set_err == 0 && !original.is_invalid() {
            let _ = session::set_thread_desktop(original);
        }
        session::close_desktop(hin);
        std::thread::sleep(Duration::from_millis(600));
    }
    append("desktop-probe 종료. 위에서 desktop=\"Winlogon\" 줄의 GetCursorPos 가 '성공(좌표 변함)'이면 로그인 화면 엣지 전환 구현 가능.");
}

/// `--desktop-probe-launch` / `--logon-switch-launch` — 세션0 SYSTEM 에서 콘솔 세션 winlogon 데스크톱으로 재투입.
pub fn run_launcher(child_args: &str) {
    append("==================================================");
    append(&format!("세션0 런처 시작 · 실행 계정={} · child={}", session::who(), child_args));
    let cs = session::active_console_session();
    append(&format!("활성 콘솔 세션 = {} (0xFFFFFFFF 면 콘솔 미연결)", cs));
    let log = |s: &str| append(s);
    match session::spawn_worker(child_args, "winsta0\\winlogon", &log) {
        Some((h, pid, s)) => {
            append(&format!(
                "CreateProcessAsUser 성공 — 세션 {} winlogon 데스크톱에 프로브 띄움 pid={}. 이제 Win+L 로 잠금화면에서 마우스를 움직여 보세요(약 100초). 아래에 그 프로브 로그가 이어집니다.",
                s, pid
            ));
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(h.handle());
            }
        }
        None => append("→ 세션 winlogon 데스크톱 진입 불가(정책/권한)"),
    }
    append("세션0 런처 종료.");
}

// ---- 예약 작업(ONSTART SYSTEM) — 상승된 인스턴스에서 ----

pub fn request_install_run() -> bool {
    run_elevated_self("--install-probe", 20000)
}
pub fn request_uninstall() -> bool {
    run_elevated_self("--uninstall-probe", 20000)
}

pub fn do_install_run() -> i32 {
    let inner = format!("\\\"{}\\\" --desktop-probe-launch", exe_path().display());
    let (rc, _) = run_hidden(
        "schtasks.exe",
        &format!("/Create /TN \"{}\" /TR \"{}\" /SC ONSTART /RU SYSTEM /RL HIGHEST /F", PROBE_TASK, inner),
        15000,
    );
    if rc != 0 {
        return rc;
    }
    run_hidden("schtasks.exe", &format!("/Run /TN \"{}\"", PROBE_TASK), 15000);
    0
}

pub fn do_uninstall() -> i32 {
    run_hidden("schtasks.exe", &format!("/Delete /TN \"{}\" /F", PROBE_TASK), 15000).0
}

pub fn is_installed() -> bool {
    run_hidden("schtasks.exe", &format!("/Query /TN \"{}\"", PROBE_TASK), 5000).0 == 0
}

/// [로그 열기] — 없으면 안내, 있으면 메모장.
pub fn open_log(owner: Option<windows::Win32::Foundation::HWND>) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK};
    let path = paths::probe_log_path();
    if !path.exists() {
        crate::util::message_box(
            owner,
            "아직 테스트 로그가 없습니다.\r\n먼저 [테스트 설치·실행] 후, 잠금화면(Win+L)에서 마우스를 움직여 보세요.",
            "LogiFlow",
            MB_OK | MB_ICONINFORMATION,
        );
        return;
    }
    crate::util::open_with("notepad.exe", &path);
}
