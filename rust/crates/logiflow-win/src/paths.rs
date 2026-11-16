//! 이름·경로. 내부 이름(LogiFlowRs…)은 시험판 때 C# 판과 겹치지 않게 정한 것을 0.16.0 기본 판에서도 그대로 쓴다
//! — 이미 깔린 rs 판이 그 자리에서 업그레이드되게(옮겨 담기 없음). 사용자에게 보이는 이름만 「LogiFlow」.

use std::path::PathBuf;

/// 제품 폴더·이름 접두.
pub const APP: &str = "LogiFlowRs";
pub const EXE_NAME: &str = "LogiFlowRs.exe";
/// 트레이 단일 인스턴스 뮤텍스.
pub const MUTEX_NAME: &str = "LogiFlowRs-6F3B2D1A-9C84-4E57-B0A3-7D52E1C9F046";
pub const SERVICE_NAME: &str = "LogiFlowRsLogonSwitch";
pub const SERVICE_DISPLAY: &str = "LogiFlow 로그인 전 전환";
pub const TRAY_TASK: &str = "LogiFlowRs-Tray";
pub const PROBE_TASK: &str = "LogiFlowRs-DesktopProbe";
pub const OLD_PRELOGON_TASK: &str = "LogiFlowRs-PreLogon";
pub const RUN_VALUE: &str = "LogiFlowRs";
pub const FIREWALL_RULE: &str = "LogiFlowRs Clipboard";
/// 트레이 IPC 창 클래스(설정 창이 찾음).
pub const TRAY_IPC_CLASS: &str = "LogiFlowRs.TrayIpc";

/// C# 판(감지·이어받기·정리 전용). 사용자 설정 폴더 `%APPDATA%\LogiFlow` 는 읽기만 하고 지우지 않는다.
pub mod legacy {
    pub const APP: &str = "LogiFlow";
    pub const EXE_NAME: &str = "LogiFlow.exe";
    pub const MUTEX_NAME: &str = "LogiFlow-A2E1C4D6-8B3F-4A91-9C72-1E5D7F0A2B34";
    pub const SERVICE_NAME: &str = "LogiFlowLogonSwitch";
    pub const TASKS: [&str; 3] = ["LogiFlow-PreLogon", "LogiFlow-DesktopProbe", "LogiFlow-Tray"];
    pub const RUN_VALUE: &str = "LogiFlow";
    pub const FIREWALL_RULE: &str = "LogiFlow Clipboard";
    /// C# 설치 파일(Inno Setup, 사용자 범위)의 제거 정보 키(HKCU).
    pub const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{B1F3C2A4-7D6E-4C58-9A21-3F0E8C4D5B60}_is1";
}

/// C# 판 머신 공유 폴더(`%ProgramData%\LogiFlow` — 워커 사본·logon-edges·로그). 정리 때 지운다.
pub fn legacy_machine_dir() -> PathBuf {
    env_dir("ProgramData", r"C:\ProgramData").join(legacy::APP)
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
/// 이전 판에서 「로그온 전에도 실행」을 쓰고 있었다는 표시 — 첫 실행 때 한 번 켤지 묻는다.
pub fn offer_prelogon_flag() -> PathBuf {
    user_dir().join("offer-prelogon.flag")
}
pub fn worker_dir() -> PathBuf {
    machine_dir().join("worker")
}
