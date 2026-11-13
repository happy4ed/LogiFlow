//! 로그인 화면 접근 테스트 결과 읽기 — `desktop-probe.log` 의 마지막 테스트 구간에서 보안 화면(잠금·로그인,
//! 데스크톱 이름 ≠ Default) 줄의 커서 읽기 결과로 판정한다. 화면에는 판정만 쉬운 말로 보여 주고,
//! 개발 용어가 든 원본 기록은 원할 때만 연다.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeVerdict {
    /// 테스트 기록이 없음.
    NotRun,
    /// 테스트는 돌았지만 잠금·로그인 화면에 간 기록이 없음.
    NoSecureScreen,
    /// 잠금·로그인 화면에서 커서를 읽지 못함 — 이 PC 에서는 로그인 화면 전환이 막혀 있음.
    Blocked,
    /// 커서는 읽었지만 움직임이 없었음 — 마우스를 움직이며 다시 테스트.
    NoMovement,
    /// 잠금·로그인 화면에서 커서 움직임을 읽음 — 로그인 화면 전환 가능.
    Works,
}

const RUN_START: &str = "desktop-probe 시작";

pub fn verdict(lines: &[String]) -> ProbeVerdict {
    let Some(start) = lines.iter().rposition(|l| l.contains(RUN_START)) else {
        return ProbeVerdict::NotRun;
    };
    let mut secure_lines = 0;
    let mut open_failed = false;
    let mut coords: Vec<String> = Vec::new();
    for l in &lines[start..] {
        if l.contains("OpenInputDesktop 실패") {
            open_failed = true;
            continue;
        }
        // 측정 줄만: desktop="이름" setThreadDesktop=… GetCursorPos=(x,y)|실패
        if !l.contains("setThreadDesktop=") {
            continue;
        }
        let Some(i) = l.find("desktop=\"") else { continue };
        let rest = &l[i + "desktop=\"".len()..];
        let Some(end) = rest.find('"') else { continue };
        if rest[..end].eq_ignore_ascii_case("Default") {
            continue;
        }
        secure_lines += 1;
        if let Some(j) = l.find("GetCursorPos=(") {
            let r = &l[j + "GetCursorPos=(".len()..];
            if let Some(k) = r.find(')') {
                let c = r[..k].to_string();
                if !coords.contains(&c) {
                    coords.push(c);
                }
            }
        }
    }
    match (secure_lines, coords.len()) {
        (0, _) if open_failed => ProbeVerdict::Blocked,
        (0, _) => ProbeVerdict::NoSecureScreen,
        (_, 0) => ProbeVerdict::Blocked,
        (_, 1) => ProbeVerdict::NoMovement,
        _ => ProbeVerdict::Works,
    }
}
