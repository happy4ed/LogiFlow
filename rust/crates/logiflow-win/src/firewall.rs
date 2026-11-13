//! 클립보드 수신 포트 방화벽 인바운드 허용 — C# `ClipFirewall` 이식(관리자 netsh, runas 상승).

use crate::paths::FIREWALL_RULE;
use crate::util::{run_elevated_self, run_hidden};

pub fn request_allow(port: i32) -> bool {
    run_elevated_self(&format!("--allow-firewall={}", port), 15000)
}

/// 상승된 인스턴스(`--allow-firewall=P`).
pub fn do_allow(port: i32) -> i32 {
    run_hidden("netsh.exe", &format!("advfirewall firewall delete rule name=\"{}\"", FIREWALL_RULE), 10000);
    run_hidden(
        "netsh.exe",
        &format!(
            "advfirewall firewall add rule name=\"{}\" dir=in action=allow protocol=TCP localport={} profile=any",
            FIREWALL_RULE, port
        ),
        10000,
    )
    .0
}

/// 제거 정리(상승된 인스턴스).
pub fn remove() {
    run_hidden("netsh.exe", &format!("advfirewall firewall delete rule name=\"{}\"", FIREWALL_RULE), 10000);
}
