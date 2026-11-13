//! LocalSystem · 자동시작 서비스 — C# `LogonSwitchService` 이식. 콜드부팅 포함 모든 부팅에서 로그인 전에 뜬다.
//! 2초마다 활성 콘솔 세션을 보고 워커 둘(winlogon = 로그온/잠금 폴링, default = 바탕화면 raw)을 감독한다.

use crate::paths::SERVICE_NAME;
use crate::session::{self, service_log, Proc, NO_SESSION};
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows_service::service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

define_windows_service!(ffi_service_main, service_main);

/// `--service` 진입점(SCM 이 호출).
pub fn run() {
    let _ = service_dispatcher::start(SERVICE_NAME, ffi_service_main);
}

fn service_main(_args: Vec<OsString>) {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let handler = move |ev| -> ServiceControlHandlerResult {
        match ev {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                stop2.store(true, Ordering::SeqCst);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate | ServiceControl::SessionChange(_) => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    let status = match service_control_handler::register(SERVICE_NAME, handler) {
        Ok(s) => s,
        Err(_) => return,
    };
    let set = |state: ServiceState, accept: ServiceControlAccept| {
        let _ = status.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accept,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::from_secs(3),
            process_id: None,
        });
    };
    set(
        ServiceState::Running,
        ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN | ServiceControlAccept::SESSION_CHANGE,
    );
    supervise(&stop);
    set(ServiceState::Stopped, ServiceControlAccept::empty());
}

fn supervise(stop: &AtomicBool) {
    service_log("supervisor 시작 (서비스 = 콜드부팅부터 상주)");
    let log = |s: &str| service_log(s);
    let mut logon = (Proc::NONE, NO_SESSION);
    let mut desk = (Proc::NONE, NO_SESSION);
    while !stop.load(Ordering::SeqCst) {
        let cur = session::active_console_session();
        if cur != NO_SESSION {
            // 세션당 워커 둘. 각 워커가 자기 입력 데스크톱일 때만 동작하므로 중복 전환이 없다.
            if !logon.0.alive() || cur != logon.1 {
                logon.0.kill();
                logon = (Proc::NONE, NO_SESSION);
                if let Some((h, _pid, s)) = session::spawn_worker("--logon-switch", "winsta0\\winlogon", &log) {
                    logon = (h, s);
                }
            }
            if !desk.0.alive() || cur != desk.1 {
                desk.0.kill();
                desk = (Proc::NONE, NO_SESSION);
                if let Some((h, _pid, s)) = session::spawn_worker("--desktop-switch", "winsta0\\default", &log) {
                    desk = (h, s);
                }
            }
        }
        // ~2초 대기(중단 시 빠르게 반응).
        for _ in 0..20 {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    logon.0.kill();
    desk.0.kill();
    service_log("supervisor 종료");
}
