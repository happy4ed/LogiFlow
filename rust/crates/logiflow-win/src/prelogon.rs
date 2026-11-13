//! 로그온 전에도 실행 — C# `PreLogon`·`PreLogonRunner` 이식 + 제거 정리(지금 판과 다른 점).
//!
//! 켜기: 워커 실행 파일을 `%ProgramData%\LogiFlowRs\worker` 로 복사(로그인 전엔 사용자 폴더가 안 올라옴) →
//! 옛 예약 작업 제거 → 서비스(LocalSystem·자동시작) 설치. 끄기: 서비스 삭제.
//! 제거 정리: 프로그램을 지울 때 서비스·워커 사본·예약 작업·방화벽 규칙까지 지운다(C# 판은 남김).

use crate::firewall;
use crate::hid::WinHidBackend;
use crate::paths::{self, EXE_NAME, OLD_PRELOGON_TASK, PROBE_TASK};
use crate::service_ctl;
use crate::session::probe_log;
use crate::util::{exe_dir, exe_path, run_elevated_self, run_hidden};
use logiflow_core::log::DiagLogger;
use logiflow_core::{scan, switcher};
use std::path::PathBuf;

/// 설치 여부 = 서비스 존재(표준 사용자 권한).
pub fn is_installed() -> bool {
    service_ctl::exists()
}

pub fn request_enable(my_host: i32) -> bool {
    run_elevated_self(&format!("--install-prelogon={}", my_host), 20000)
}

pub fn request_disable() -> bool {
    run_elevated_self("--uninstall-prelogon", 20000)
}

fn schtasks(args: &str) -> i32 {
    let (rc, out) = run_hidden("schtasks.exe", args, 15000);
    if rc != 0 {
        probe_log(&format!("schtasks 실패({}): {} | {}", rc, args, out.replace(['\r', '\n'], " ").trim()));
    }
    rc
}

/// 워커 실행 파일을 머신 공용 폴더로 복사하고 그 경로를 돌려준다. 실패하면 원본 경로.
fn copy_worker_to_machine() -> PathBuf {
    let src = exe_path();
    let dst_dir = paths::worker_dir();
    let _ = std::fs::create_dir_all(&dst_dir);
    let dst = dst_dir.join(EXE_NAME);
    if std::fs::copy(&src, &dst).is_ok() || dst.exists() {
        return dst;
    }
    src
}

/// 상승된 인스턴스(`--install-prelogon=N`).
pub fn do_install(_my_host: i32) -> i32 {
    // 서비스가 돌고 있으면 먼저 멈춰 워커 사본을 새 판으로 덮어쓸 수 있게 한다.
    if service_ctl::exists() {
        service_ctl::stop_wait();
    }
    let exe = copy_worker_to_machine();
    let _ = schtasks(&format!("/Delete /TN \"{}\" /F", OLD_PRELOGON_TASK));
    let rc = service_ctl::install(&exe);
    probe_log(&format!("서비스 설치({}, 자동시작) rc={} exe={}", paths::SERVICE_NAME, rc, exe.display()));
    rc
}

/// 상승된 인스턴스(`--uninstall-prelogon`).
pub fn do_uninstall() -> i32 {
    let _ = schtasks(&format!("/Delete /TN \"{}\" /F", OLD_PRELOGON_TASK));
    let rc = service_ctl::uninstall();
    probe_log(&format!("서비스 제거 rc={}", rc));
    rc
}

/// 제거 프로그램이 부름(`--uninstall-cleanup`, 일반 권한). 사용자 범위 정리 후, 머신 범위에 남은 게 있으면
/// 그때만 UAC 상승(`--uninstall-machine`). 사용자 설정(`%APPDATA%\LogiFlowRs`)은 보존.
pub fn uninstall_cleanup() -> i32 {
    crate::autostart::remove_all();
    // 방화벽 규칙만 따로 허용한 경우(로그온 전 실행은 안 켬)도 머신 범위 잔존물이다.
    let machine_left = service_ctl::exists() || paths::machine_dir().exists() || crate::probe::is_installed() || firewall::exists();
    if machine_left && !run_elevated_self("--uninstall-machine", 60000) {
        return 1;
    }
    0
}

/// 상승된 인스턴스(`--uninstall-machine`) — 서비스 중지·삭제, 예약 작업·방화벽 규칙 삭제, 머신 폴더 삭제.
pub fn uninstall_machine() -> i32 {
    let rc = service_ctl::uninstall();
    run_hidden("schtasks.exe", &format!("/Delete /TN \"{}\" /F", PROBE_TASK), 15000);
    run_hidden("schtasks.exe", &format!("/Delete /TN \"{}\" /F", OLD_PRELOGON_TASK), 15000);
    firewall::remove();
    // 서비스가 멈춘 뒤에도 워커가 잠시 남을 수 있어 몇 번 재시도.
    for _ in 0..10 {
        if !paths::machine_dir().exists() || std::fs::remove_dir_all(paths::machine_dir()).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    // 덜 지워졌으면 실패로 돌려준다(머신 폴더는 지웠으니 여기선 로그를 남기지 않음 — 로그가 폴더를 되살림).
    // (서비스는 「삭제 예약」 상태로 잠시 남을 수 있어 rc 로만 판단.)
    if rc == 0 && paths::machine_dir().exists() {
        return 1;
    }
    rc
}

/// `--prelogon-pull=N`(옛 경로) — UI 없이 이 PC 채널로 당기기만.
pub fn run_pull(my_host: i32) {
    let log = DiagLogger::new(&exe_dir(), true);
    log.log(&format!("=== 로그온 전 당기기 시작 (myHost 채널 {}) ===", my_host + 1));
    if my_host < 0 {
        log.log("myHost 미지정 — 중단");
        return;
    }
    let sw = scan::switchable(&scan::scan(&WinHidBackend, &log));
    let sent = switcher::switch_all(&WinHidBackend, &sw, my_host, &log);
    log.log(&format!("로그온 전 당기기 전송: {}개", sent));
}
