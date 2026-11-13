//! 이름·경로. 러스트 시험판은 모든 이름을 C# 판과 다르게 해 나란히 설치된다(계획서 6절 3).

use std::path::PathBuf;

/// 제품 폴더·이름 접두.
pub const APP: &str = "LogiFlowRs";
pub const EXE_NAME: &str = "LogiFlowRs.exe";
/// 트레이 단일 인스턴스 뮤텍스.
pub const MUTEX_NAME: &str = "LogiFlowRs-6F3B2D1A-9C84-4E57-B0A3-7D52E1C9F046";
pub const SERVICE_NAME: &str = "LogiFlowRsLogonSwitch";
pub const SERVICE_DISPLAY: &str = "LogiFlow 로그온 전환 (러스트 시험판)";
pub const TRAY_TASK: &str = "LogiFlowRs-Tray";
pub const PROBE_TASK: &str = "LogiFlowRs-DesktopProbe";
pub const OLD_PRELOGON_TASK: &str = "LogiFlowRs-PreLogon";
pub const RUN_VALUE: &str = "LogiFlowRs";
pub const FIREWALL_RULE: &str = "LogiFlowRs Clipboard";
/// 트레이 IPC 창 클래스(설정 창이 찾음).
pub const TRAY_IPC_CLASS: &str = "LogiFlowRs.TrayIpc";

/// C# 판(감지·이어받기 전용, 쓰지 않음).
pub mod legacy {
    pub const APP: &str = "LogiFlow";
    pub const MUTEX_NAME: &str = "LogiFlow-A2E1C4D6-8B3F-4A91-9C72-1E5D7F0A2B34";
    pub const SERVICE_NAME: &str = "LogiFlowLogonSwitch";
}

fn env_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(fallback))
}

/// `%APPDATA%\LogiFlowRs` — 사용자 설정·기기 캐시.
pub fn user_dir() -> PathBuf {
    env_dir("APPDATA", r"C:\Users\Default\AppData\Roaming").join(APP)
}
pub fn config_path() -> PathBuf {
    user_dir().join("config.txt")
}
pub fn devices_path() -> PathBuf {
    user_dir().join("devices.txt")
}

/// C# 판 사용자 폴더(`%APPDATA%\LogiFlow`).
pub fn legacy_user_dir() -> PathBuf {
    env_dir("APPDATA", r"C:\Users\Default\AppData\Roaming").join(legacy::APP)
}

/// `%ProgramData%\LogiFlowRs` — 머신 공유(워커가 읽음).
pub fn machine_dir() -> PathBuf {
    env_dir("ProgramData", r"C:\ProgramData").join(APP)
}
pub fn logon_edges_path() -> PathBuf {
    machine_dir().join("logon-edges.txt")
}
pub fn lock_flag_path() -> PathBuf {
    machine_dir().join("locked.flag")
}
pub fn probe_log_path() -> PathBuf {
    machine_dir().join("desktop-probe.log")
}
pub fn worker_dir() -> PathBuf {
    machine_dir().join("worker")
}
