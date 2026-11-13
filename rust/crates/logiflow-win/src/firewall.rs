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

/// 설정 창 「허용 해제」 — 관리자 승인(UAC) 후 규칙 삭제.
pub fn request_remove() -> bool {
    run_elevated_self("--remove-firewall", 15000)
}

/// 상승된 인스턴스(`--remove-firewall`). 지우기 명령이 성공했거나, 실패했어도 규칙이 없으면 0.
pub fn do_remove() -> i32 {
    let (rc, _) = run_hidden("netsh.exe", &format!("advfirewall firewall delete rule name=\"{}\"", FIREWALL_RULE), 10000);
    if rc == 0 || !exists() {
        0
    } else {
        1
    }
}

/// 이 포트가 우리 규칙으로 열려 있나 — 규칙이 있어도 포트가 다르면(설정 파일에서 포트를 바꾼 경우) false.
/// 조회 실패(방화벽 서비스 꺼짐 등)도 false 로 보고 「허용」을 다시 누를 수 있게 둔다.
pub fn allowed_for_port(port: i32) -> bool {
    let (rc, out) = run_hidden("netsh.exe", &format!("advfirewall firewall show rule name=\"{}\"", FIREWALL_RULE), 10000);
    let p = port.to_string();
    rc == 0 && out.split(|c: char| !c.is_ascii_digit()).any(|tok| tok == p)
}

/// 제거 정리(상승된 인스턴스).
pub fn remove() {
    run_hidden("netsh.exe", &format!("advfirewall firewall delete rule name=\"{}\"", FIREWALL_RULE), 10000);
}

/// 규칙이 남아 있나(조회는 일반 권한으로 됨 — 없으면 netsh 가 「규칙 없음」과 함께 0 이 아닌 코드).
pub fn exists() -> bool {
    run_hidden("netsh.exe", &format!("advfirewall firewall show rule name=\"{}\"", FIREWALL_RULE), 10000).0 == 0
}
