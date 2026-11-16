//! C# 판과 나란히 — 감지(중복 전환 막기)와 첫 실행 설정 이어받기(계획서 6절 4·5).
//! C# 판 파일·설정은 읽기만 하고 절대 쓰지 않는다.

use crate::dpapi;
use crate::paths::{self, legacy};
use crate::service_ctl;
use crate::util::wide;
use logiflow_core::config::AppConfig;
use logiflow_core::log::DiagLogger;
use windows::core::PCWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};

/// C# 트레이가 이 세션에서 돌고 있나(단일 인스턴스 뮤텍스 존재).
pub fn csharp_tray_running() -> bool {
    let n = wide(legacy::MUTEX_NAME);
    unsafe {
        match OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(n.as_ptr())) {
            Ok(h) => {
                let _ = CloseHandle(h);
                true
            }
            Err(_) => false,
        }
    }
}

pub fn csharp_service_exists() -> bool {
    service_ctl::legacy_exists()
}

/// 가장자리 감시를 켜면 C# 판과 중복 전환이 날 수 있는가.
pub fn conflict() -> bool {
    csharp_tray_running() || csharp_service_exists()
}

/// 첫 실행이면 C# 판 config.txt·devices.txt 를 옮겨 온다. 시험판 한정: preLogon=0 · autostart=0.
/// 암호문구는 DPAPI 로 옮겨 담고, C# 원본 평문 파일은 건드리지 않는다.
pub fn import_if_first_run(log: &DiagLogger) -> bool {
    let ours = paths::config_path();
    if ours.exists() {
        return false;
    }
    let theirs = paths::legacy_user_dir().join("config.txt");
    if !theirs.exists() {
        return false;
    }
    let mut cfg = AppConfig::load(&theirs, None);
    cfg.pre_logon = false;
    cfg.auto_start = false;
    let saved = dpapi::with_codec(|c| cfg.save(&ours, Some(c))).is_ok();
    let dev_src = paths::legacy_user_dir().join("devices.txt");
    let dev_dst = paths::devices_path();
    if dev_src.exists() && !dev_dst.exists() {
        let _ = std::fs::copy(&dev_src, &dev_dst);
    }
    log.log(&format!(
        "C# 판 설정 이어받기: {} → {} ({}) · preLogon=0 · autostart=0",
        theirs.display(),
        ours.display(),
        if saved { "성공" } else { "실패" }
    ));
    saved
}

// ===================== 기존 LogiFlow(C# 판) 정리 =====================
//
// C# 설치 파일은 「로그온 전에도 실행」을 되돌리지 않는다 — 서비스 LogiFlowLogonSwitch 가 워커 사본
// (%ProgramData%\LogiFlow\worker\LogiFlow.exe)으로 돌고 있어 앱을 지워도 서비스가 남는다(회장님 PC 11-16).
// 정리: ① 사용자 쪽(관리자 불필요) — C# 앱이 아직 깔려 있으면 그 제거 프로그램을 조용히 실행, HKCU Run 값 삭제.
//       ② 머신 쪽(관리자, UAC 한 번) — 남은 C# 트레이(뮤텍스로 확인)·워커 종료, 서비스 중지·삭제, 예약 작업 3개,
//          %ProgramData%\LogiFlow, 방화벽 규칙.
// 사용자 설정 %APPDATA%\LogiFlow 는 절대 지우지 않는다. 단계마다 기록을 남긴다.

use crate::paths::legacy as lg;
use crate::util::{run_elevated_self, run_hidden};
use std::path::Path;
use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};

/// 남은 것(빠른 확인만 — 예약 작업 조회는 느려서 정리 때 지우기만 한다).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Leftovers {
    /// C# 앱이 설치돼 있음(제거 정보 키).
    pub installed: bool,
    pub tray_running: bool,
    pub service: bool,
    pub machine_dir: bool,
    pub run_value: bool,
}

impl Leftovers {
    pub fn any(&self) -> bool {
        self.installed || self.tray_running || self.service || self.machine_dir || self.run_value
    }
    /// 가장자리 전환·로그인 전 실행과 겹치는 것(트레이 또는 서비스).
    pub fn conflicts(&self) -> bool {
        self.tray_running || self.service
    }
    /// 화면에 보일 한 줄 — 무엇이 남았는지.
    pub fn describe(&self) -> String {
        let mut v = Vec::new();
        if self.installed {
            v.push("앱");
        }
        if self.tray_running {
            v.push("실행 중인 트레이");
        }
        if self.service {
            v.push("「로그온 전에도 실행」 서비스");
        }
        if self.machine_dir || self.run_value {
            v.push("남은 파일·자동 시작 항목");
        }
        v.join(" · ")
    }
}

pub fn leftovers() -> Leftovers {
    Leftovers {
        installed: uninstall_string().is_some(),
        tray_running: csharp_tray_running(),
        service: csharp_service_exists(),
        machine_dir: crate::paths::legacy_machine_dir().exists(),
        run_value: reg_sz(lg_run_key(), lg::RUN_VALUE).is_some(),
    }
}

fn lg_run_key() -> &'static str {
    r"Software\Microsoft\Windows\CurrentVersion\Run"
}

fn reg_sz(subkey: &str, value: &str) -> Option<String> {
    let k = wide(subkey);
    let v = wide(value);
    let mut buf = vec![0u16; 2048];
    let mut size = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(v.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
    };
    if r.is_err() {
        return None;
    }
    let n = (size as usize / 2).min(buf.len());
    Some(crate::util::from_wide(&buf[..n]))
}

/// C# 제거 프로그램 경로(따옴표 없는 unins000.exe 경로).
fn uninstall_string() -> Option<String> {
    let s = reg_sz(lg::UNINSTALL_KEY, "UninstallString")?;
    let s = s.trim().trim_matches('"').to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// ① 사용자 쪽 정리(관리자 불필요). 기록 줄들을 돌려준다.
pub fn cleanup_user(log: &DiagLogger) -> Vec<String> {
    let mut out = Vec::new();
    let mut say = |s: String| {
        log.log(&format!("[기존 LogiFlow 정리] {}", s));
        out.push(s);
    };
    if let Some(unins) = uninstall_string() {
        say(format!("C# 앱 제거 프로그램 실행: {}", unins));
        let (rc, _) = run_hidden(&unins, "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART", 60000);
        // Inno 제거 프로그램은 임시 사본으로 다시 떠서 끝나므로, 제거 정보 키가 사라질 때까지 기다린다.
        let start = std::time::Instant::now();
        while uninstall_string().is_some() && start.elapsed() < std::time::Duration::from_secs(60) {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        let gone = uninstall_string().is_none();
        say(format!(
            "C# 앱 제거 {} (rc={})",
            if gone {
                "완료"
            } else {
                "안 끝남 — 설정 → 앱에서 지워 주세요"
            },
            rc
        ));
    } else {
        say("C# 앱은 이미 제거됨".into());
    }
    if reg_sz(lg_run_key(), lg::RUN_VALUE).is_some() {
        let k = wide(lg_run_key());
        let v = wide(lg::RUN_VALUE);
        let ok = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(v.as_ptr())) }.is_ok();
        say(format!("로그인 자동 시작(Run 「{}」) 삭제 {}", lg::RUN_VALUE, if ok { "완료" } else { "실패" }));
    }
    out
}

/// ② 머신 쪽 정리(관리자). `%APPDATA%\LogiFlow` 는 건드리지 않는다. 0 = 남은 것 없음.
pub fn cleanup_machine() -> i32 {
    use crate::session::probe_log;
    let say = |s: &str| probe_log(&format!("[기존 LogiFlow 정리] {}", s));
    say("시작(관리자)");
    let dir = crate::paths::legacy_machine_dir();
    // 트레이는 아직 돌 때만(뮤텍스), 워커는 C# 머신 폴더 안에서 도는 것만 끈다. 러스트 판(LogiFlowRs.exe)은 건드리지 않음.
    let tray = csharp_tray_running();
    let killed = kill_legacy_processes(tray, &dir);
    say(&format!("C# 프로세스 종료 {}개(트레이 실행 중={})", killed, tray));
    let rc = crate::service_ctl::uninstall_legacy();
    say(&format!("서비스 {} 중지·삭제 rc={}", lg::SERVICE_NAME, rc));
    for t in lg::TASKS {
        let (r, _) = run_hidden("schtasks.exe", &format!("/Delete /TN \"{}\" /F", t), 15000);
        say(&format!("예약 작업 {} 삭제 rc={}", t, r));
    }
    let (r, _) = run_hidden("netsh.exe", &format!("advfirewall firewall delete rule name=\"{}\"", lg::FIREWALL_RULE), 10000);
    say(&format!("방화벽 규칙 「{}」 삭제 rc={}(없으면 1)", lg::FIREWALL_RULE, r));
    // 서비스가 멈춘 뒤에도 워커가 잠시 남을 수 있어 몇 번 다시 시도.
    for i in 0..10 {
        if !dir.exists() || std::fs::remove_dir_all(&dir).is_ok() {
            break;
        }
        if i == 4 {
            kill_legacy_processes(false, &dir);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    say(&format!("{} 삭제 {}", dir.display(), if dir.exists() { "실패" } else { "완료" }));
    // 같은 사용자로 상승된 경우 Run 값도 한 번 더(다른 관리자 계정이면 그 계정 것이라 영향 없음).
    let k = wide(lg_run_key());
    let v = wide(lg::RUN_VALUE);
    let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(v.as_ptr())) };
    let left = crate::service_ctl::legacy_exists() || dir.exists();
    say(if left { "끝 — 남은 것 있음" } else { "끝 — 남은 것 없음" });
    if left {
        1
    } else {
        0
    }
}

/// 설정 창에서 ① 을 마친 뒤: 머신 쪽에 남은 게 있으면 ② 를 UAC 로. (남은 것 없음, 기록 줄).
pub fn request_cleanup_machine_only() -> (bool, Vec<String>) {
    let mut lines = Vec::new();
    let need_machine = {
        let l = leftovers();
        l.service || l.machine_dir || l.tray_running
    };
    if need_machine {
        let ok = run_elevated_self("--cleanup-legacy-machine", 60000);
        lines.push(if ok {
            "서비스·예약 작업·남은 파일 정리 완료".into()
        } else {
            "관리자 승인이 취소됐거나 일부를 지우지 못했습니다".into()
        });
    }
    let ok = !leftovers().conflicts() && !crate::paths::legacy_machine_dir().exists();
    (ok, lines)
}

/// 이름이 LogiFlow.exe 인 프로세스 중 — 트레이(실행 중일 때) 또는 C# 머신 폴더 안의 워커를 끈다. 끈 개수.
fn kill_legacy_processes(include_tray: bool, machine_dir: &Path) -> usize {
    use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    };
    let mut n = 0;
    let dir = machine_dir.to_string_lossy().to_ascii_lowercase();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return 0;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let name = crate::util::from_wide(&e.szExeFile);
            if name.eq_ignore_ascii_case(lg::EXE_NAME) {
                if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE, false, e.th32ProcessID) {
                    let mut buf = [0u16; 1024];
                    let mut len = buf.len() as u32;
                    let path = if QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len).is_ok() {
                        String::from_utf16_lossy(&buf[..len as usize]).to_ascii_lowercase()
                    } else {
                        String::new()
                    };
                    let worker = !dir.is_empty() && path.starts_with(&dir);
                    if (include_tray || worker) && TerminateProcess(h, 1).is_ok() {
                        n += 1;
                    }
                    let _ = CloseHandle(h);
                }
            }
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    n
}
