//! 윈도우 서비스 설치/삭제/존재 확인 — C# `ServiceControl` 이식(windows-service 크레이트).
//! 설치·삭제는 관리자(상승된 `--install-prelogon` / `--uninstall-prelogon`), 존재 확인은 표준 사용자 권한.

use crate::paths::{legacy, SERVICE_DISPLAY, SERVICE_NAME};
use std::ffi::OsString;
use std::path::Path;
use std::time::{Duration, Instant};
use windows_service::service::{ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState, ServiceType};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

const ERROR_SERVICE_EXISTS: i32 = 1073;
const ERROR_SERVICE_MARKED_FOR_DELETE: i32 = 1072;

fn code(e: &windows_service::Error) -> i32 {
    match e {
        windows_service::Error::Winapi(io) => io.raw_os_error().unwrap_or(1),
        _ => 1,
    }
}

fn exists_named(name: &str) -> bool {
    let m = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(m) => m,
        Err(_) => return false,
    };
    m.open_service(name, ServiceAccess::QUERY_STATUS).is_ok()
}

/// 러스트 판 서비스가 등록돼 있나(토글 상태 표시용).
pub fn exists() -> bool {
    exists_named(SERVICE_NAME)
}

/// C# 판 서비스가 등록돼 있나(중복 전환 막기, 계획서 6절 4).
pub fn legacy_exists() -> bool {
    exists_named(legacy::SERVICE_NAME)
}

/// 설치 + 즉시 시작(관리자). 0 성공, 그 외 Win32 오류 코드.
pub fn install(exe: &Path) -> i32 {
    let m = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE) {
        Ok(m) => m,
        Err(e) => return code(&e).max(5),
    };
    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe.to_path_buf(),
        launch_arguments: vec![OsString::from("--service")],
        dependencies: vec![],
        account_name: None, // LocalSystem
        account_password: None,
    };
    let svc = match m.create_service(&info, ServiceAccess::all()) {
        Ok(s) => s,
        Err(e) if code(&e) == ERROR_SERVICE_EXISTS => {
            // 이미 있으면 실행 파일 경로를 맞춘 뒤 시작만(다음 부팅 자동 상주가 목적).
            match m.open_service(SERVICE_NAME, ServiceAccess::all()) {
                Ok(s) => {
                    let _ = s.change_config(&info);
                    s
                }
                Err(_) => return ERROR_SERVICE_EXISTS,
            }
        }
        Err(e) => return code(&e),
    };
    let _ = svc.start::<&str>(&[]); // 실패해도 다음 부팅에 자동 시작
    0
}

/// 중지 + 삭제(관리자). 0 성공(또는 이미 없음). 중지 완료를 최대 10초 기다린다(워커 사본 지우기 전).
pub fn uninstall() -> i32 {
    let m = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(m) => m,
        Err(e) => return code(&e).max(5),
    };
    let svc = match m.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE) {
        Ok(s) => s,
        Err(_) => return 0, // 이미 없음
    };
    let _ = svc.stop();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        match svc.query_status() {
            Ok(st) if st.current_state == ServiceState::Stopped => break,
            Ok(_) => std::thread::sleep(Duration::from_millis(200)),
            Err(_) => break,
        }
    }
    match svc.delete() {
        Ok(()) => 0,
        Err(e) if code(&e) == ERROR_SERVICE_MARKED_FOR_DELETE => 0,
        Err(e) => code(&e),
    }
}

/// 실행 중이면 멈추고 멈출 때까지(최대 10초) 기다린다(관리자).
pub fn stop_wait() {
    let m = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(m) => m,
        Err(_) => return,
    };
    if let Ok(svc) = m.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP) {
        let _ = svc.stop();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            match svc.query_status() {
                Ok(st) if st.current_state == ServiceState::Stopped => break,
                Ok(_) => std::thread::sleep(Duration::from_millis(200)),
                Err(_) => break,
            }
        }
    }
}
