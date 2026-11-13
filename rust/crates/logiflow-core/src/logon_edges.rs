//! 로그온/바탕화면 워커(SYSTEM)가 읽는 머신 공유 엣지 설정(`logon-edges.txt`) — C# `LogonEdges` 형식.
//! 좌표는 저장하지 않고 모니터 식별(이름/순번)만 저장 — 워커가 자기 모니터 목록으로 경계를 구한다.
//!
//! 지금 판과 다른 점: `dragGuard=` 줄 추가(바탕화면 워커가 「드래그 중 전환 안 함」을 따르게).

use crate::config::AppConfig;
use crate::edge::DEFAULT_DWELL_MS;
use crate::log::newline;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogonEdge {
    pub device: String,
    /// 보조 매칭용 모니터 순번(-1 = 없음).
    pub index: i32,
    pub edge: i32,
    pub channel: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogonEdgeFile {
    pub dwell_ms: i32,
    pub my_host: i32,
    pub debug: bool,
    pub drag_guard: bool,
    pub edges: Vec<LogonEdge>,
}

impl Default for LogonEdgeFile {
    fn default() -> Self {
        LogonEdgeFile {
            dwell_ms: DEFAULT_DWELL_MS,
            my_host: -1,
            debug: false,
            drag_guard: true,
            edges: Vec::new(),
        }
    }
}

/// `monitor_names` = 모니터 열거 순서의 장치 이름(C# Screen.AllScreens 순서).
pub fn serialize(cfg: &AppConfig, monitor_names: &[String]) -> String {
    let nl = newline();
    let mut s = String::new();
    s.push_str(&format!("dwellMs={}{}", cfg.dwell_ms, nl));
    s.push_str(&format!("myHost={}{}", cfg.my_host, nl));
    s.push_str(&format!("debug={}{}", if cfg.debug_mode { "1" } else { "0" }, nl));
    s.push_str(&format!("dragGuard={}{}", if cfg.drag_guard { "1" } else { "0" }, nl));
    for a in &cfg.attachments {
        let idx = monitor_names.iter().position(|n| *n == a.monitor_device_name).map(|i| i as i32).unwrap_or(-1);
        s.push_str(&format!("edge\t{}\t{}\t{}\t{}{}", a.monitor_device_name, idx, a.edge as i32, a.channel, nl));
    }
    s
}

pub fn parse(lines: &[String]) -> LogonEdgeFile {
    let mut f = LogonEdgeFile::default();
    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(v) = line.strip_prefix("dwellMs=") {
            if let Ok(d) = v.parse() {
                f.dwell_ms = d;
            }
            continue;
        }
        if let Some(v) = line.strip_prefix("myHost=") {
            if let Ok(m) = v.parse() {
                f.my_host = m;
            }
            continue;
        }
        if let Some(v) = line.strip_prefix("debug=") {
            f.debug = v == "1";
            continue;
        }
        if let Some(v) = line.strip_prefix("dragGuard=") {
            f.drag_guard = v == "1";
            continue;
        }
        if line.starts_with("edge\t") {
            let p: Vec<&str> = line.split('\t').collect();
            if p.len() >= 5 {
                f.edges.push(LogonEdge {
                    device: p[1].to_string(),
                    index: p[2].parse().unwrap_or(0), // C# TryParse 실패 = 0
                    edge: p[3].parse().unwrap_or(0),
                    channel: p[4].parse().unwrap_or(0),
                });
            }
        }
    }
    f
}
