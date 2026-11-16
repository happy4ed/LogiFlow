//! 파일 자리 — 설정·기기 캐시·기록은 `~/Library/Application Support/LogiFlowRs`(앱 번들 안은 쓰지 않는다).
//! 나란히 설치: Swift 판(번들 id `com.happy4ed.logiflow`)과 이름·자리가 겹치지 않는다.

use std::path::PathBuf;

pub const APP: &str = "LogiFlowRs";
pub const BUNDLE_ID: &str = "com.happy4ed.logiflow.rs";
/// Swift 판 UserDefaults 도메인(이어받기만 읽는다).
pub const SWIFT_DOMAIN: &str = "com.happy4ed.logiflow";
pub const LAUNCH_AGENT_LABEL: &str = "com.happy4ed.logiflow.rs";

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn support_dir() -> PathBuf {
    home().join("Library/Application Support").join(APP)
}

pub fn config_path() -> PathBuf {
    support_dir().join("config.txt")
}

pub fn devices_path() -> PathBuf {
    support_dir().join("devices.txt")
}

pub fn log_dir() -> PathBuf {
    support_dir().join("logs")
}

/// 상주 앱 IPC 소켓(설정 창 → 상주 앱).
pub fn agent_socket() -> PathBuf {
    support_dir().join("agent.sock")
}

/// 설정 창 IPC 소켓(상주 앱 → 설정 창). 프로세스 번호마다 하나.
pub fn ui_socket(pid: u32) -> PathBuf {
    support_dir().join(format!("ui-{}.sock", pid))
}

/// 상주 앱 하나만 — 이 파일의 잠금(flock)으로.
pub fn agent_lock() -> PathBuf {
    support_dir().join("agent.lock")
}

pub fn launch_agent_path() -> PathBuf {
    home().join("Library/LaunchAgents").join(format!("{}.plist", LAUNCH_AGENT_LABEL))
}

/// Windows 의 로그인 화면 테스트 기록 자리(Mac 에는 없음 — 설정 창이 묻지 않는다).
pub fn probe_log_path() -> PathBuf {
    support_dir().join("desktop-probe.log")
}

pub fn ensure_support_dir() {
    let _ = std::fs::create_dir_all(support_dir());
}
