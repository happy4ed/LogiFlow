//! 앱 설정(`config.txt`, key=value) — C# `AppConfig` 형식 그대로 읽고 쓴다.
//!
//! 지금 판과 다른 점 1건: 클립보드 암호문구를 평문 `clipSecret=` 대신 보호된 값
//! `clipSecretDpapi=<hex>`(Windows DPAPI 사용자 범위)로 저장한다. 평문 키가 들어 있는 파일(C# 판에서
//! 이어받은 것)도 읽으며, 다음 저장 때 보호된 값으로 옮겨 담는다.

use crate::edge::{HandoffEdge, DEFAULT_DWELL_MS};
use crate::log::{read_lines, write_utf8_bom};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    System = 0,
    Light = 1,
    Dark = 2,
}

impl ThemeMode {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(ThemeMode::System),
            1 => Some(ThemeMode::Light),
            2 => Some(ThemeMode::Dark),
            _ => None,
        }
    }
}

/// 엣지 변 → 채널 매핑 1건.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeAttachment {
    /// 0-base 호스트 인덱스(채널 N → N-1).
    pub channel: i32,
    /// 모니터 장치 이름(예: `\\.\DISPLAY1`).
    pub monitor_device_name: String,
    pub edge: HandoffEdge,
}

/// 암호문구 보호(저장)·해제(읽기) 함수. Windows = DPAPI. 없으면 평문으로 둔다(2단계 OS).
pub struct SecretCodec<'a> {
    pub protect: &'a dyn Fn(&str) -> Option<String>,
    pub unprotect: &'a dyn Fn(&str) -> Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppConfig {
    pub edge_switch_enabled: bool,
    pub dwell_ms: i32,
    pub auto_start: bool,
    pub drag_guard: bool,
    pub diag_log_enabled: bool,
    pub pre_logon: bool,
    pub my_host: i32,
    pub font_offset: i32,
    pub clip_enabled: bool,
    pub clip_peer_host: String,
    pub clip_port: i32,
    /// 메모리 안에서는 평문. 파일에는 보호된 값으로만 쓴다(SecretCodec 있을 때).
    pub clip_secret: String,
    pub debug_mode: bool,
    pub theme: ThemeMode,
    pub attachments: Vec<EdgeAttachment>,
    /// 채널(0-base) → 별칭. 넣은 순서 유지(C# Dictionary 열거 순서와 같게).
    pub aliases: Vec<(i32, String)>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            edge_switch_enabled: true,
            dwell_ms: DEFAULT_DWELL_MS,
            auto_start: true,
            drag_guard: true,
            diag_log_enabled: true,
            pre_logon: false,
            my_host: -1,
            font_offset: 1,
            clip_enabled: false,
            clip_peer_host: String::new(),
            clip_port: 48737,
            clip_secret: String::new(),
            debug_mode: false,
            theme: ThemeMode::System,
            attachments: Vec::new(),
            aliases: Vec::new(),
        }
    }
}

fn b(v: bool) -> &'static str {
    if v {
        "1"
    } else {
        "0"
    }
}

impl AppConfig {
    pub fn alias_for(&self, channel: i32) -> String {
        match self.aliases.iter().find(|(c, _)| *c == channel) {
            Some((_, v)) if !v.is_empty() => v.clone(),
            _ => format!("채널 {}", channel + 1),
        }
    }

    pub fn set_alias(&mut self, channel: i32, value: &str) {
        if value.is_empty() {
            self.aliases.retain(|(c, _)| *c != channel);
        } else if let Some(e) = self.aliases.iter_mut().find(|(c, _)| *c == channel) {
            e.1 = value.to_string();
        } else {
            self.aliases.push((channel, value.to_string()));
        }
    }

    /// 줄 목록에서 읽는다. 잘못된 값은 기본값 유지(C# 과 같은 범위 검사).
    pub fn parse(lines: &[String], codec: Option<&SecretCodec<'_>>) -> AppConfig {
        let mut cfg = AppConfig::default();
        let mut protected_secret: Option<String> = None;
        for raw in lines {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let eq = match line.find('=') {
                Some(e) => e,
                None => continue,
            };
            let key = line[..eq].trim();
            let val = line[eq + 1..].trim();
            let int = |s: &str| s.parse::<i32>().ok();
            match key {
                "edgeEnabled" => cfg.edge_switch_enabled = val == "1",
                "dwellMs" => {
                    if let Some(d) = int(val).filter(|d| (50..=3000).contains(d)) {
                        cfg.dwell_ms = d;
                    }
                }
                "autostart" => cfg.auto_start = val == "1",
                "dragGuard" => cfg.drag_guard = val == "1",
                "diagLog" => cfg.diag_log_enabled = val == "1",
                "preLogon" => cfg.pre_logon = val == "1",
                "myHost" => {
                    if let Some(m) = int(val).filter(|m| (-1..=15).contains(m)) {
                        cfg.my_host = m;
                    }
                }
                "fontOffset" => {
                    if let Some(f) = int(val).filter(|f| (-2..=6).contains(f)) {
                        cfg.font_offset = f;
                    }
                }
                "clipEnabled" => cfg.clip_enabled = val == "1",
                "clipHost" => cfg.clip_peer_host = val.to_string(),
                "clipPort" => {
                    if let Some(p) = int(val).filter(|p| (1..=65535).contains(p)) {
                        cfg.clip_port = p;
                    }
                }
                "clipSecret" => cfg.clip_secret = val.to_string(),
                "clipSecretDpapi" => protected_secret = Some(val.to_string()),
                "debug" => cfg.debug_mode = val == "1",
                "theme" => {
                    if let Some(t) = int(val).and_then(ThemeMode::from_i32) {
                        cfg.theme = t;
                    }
                }
                "alias" => {
                    let ap: Vec<&str> = val.split('\t').collect();
                    if ap.len() >= 2 {
                        if let Ok(ac) = ap[0].parse::<i32>() {
                            // C#: cfg.Aliases[ac] = ap[1] (빈 값도 저장).
                            if let Some(e) = cfg.aliases.iter_mut().find(|(c, _)| *c == ac) {
                                e.1 = ap[1].to_string();
                            } else {
                                cfg.aliases.push((ac, ap[1].to_string()));
                            }
                        }
                    }
                }
                "attach" => {
                    if let Some(a) = parse_attach(val) {
                        cfg.attachments.push(a);
                    }
                }
                _ => {}
            }
        }
        if let (Some(p), Some(c)) = (protected_secret, codec) {
            if let Some(s) = (c.unprotect)(&p) {
                cfg.clip_secret = s;
            }
        }
        cfg
    }

    /// 파일 내용. codec 이 있으면 암호문구를 보호된 값으로만 쓴다(보호 실패 시 암호문구는 쓰지 않음).
    /// codec 이 없으면 평문(설정 창↔트레이 사이 IPC·2단계 OS 용).
    pub fn serialize(&self, codec: Option<&SecretCodec<'_>>) -> String {
        let nl = crate::log::newline();
        let mut s = String::new();
        let mut line = |l: String| {
            s.push_str(&l);
            s.push_str(nl);
        };
        line("# LogiFlow 설정".into());
        line(format!("edgeEnabled={}", b(self.edge_switch_enabled)));
        line(format!("dwellMs={}", self.dwell_ms));
        line(format!("autostart={}", b(self.auto_start)));
        line(format!("dragGuard={}", b(self.drag_guard)));
        line(format!("diagLog={}", b(self.diag_log_enabled)));
        line(format!("preLogon={}", b(self.pre_logon)));
        line(format!("myHost={}", self.my_host));
        line(format!("fontOffset={}", self.font_offset));
        line(format!("clipEnabled={}", b(self.clip_enabled)));
        line(format!("clipHost={}", self.clip_peer_host));
        line(format!("clipPort={}", self.clip_port));
        match codec {
            Some(c) => {
                line("clipSecret=".into());
                if !self.clip_secret.is_empty() {
                    if let Some(p) = (c.protect)(&self.clip_secret) {
                        line(format!("clipSecretDpapi={}", p));
                    }
                }
            }
            None => line(format!("clipSecret={}", self.clip_secret)),
        }
        line(format!("debug={}", b(self.debug_mode)));
        line(format!("theme={}", self.theme as i32));
        for (c, v) in &self.aliases {
            if !v.is_empty() {
                line(format!("alias={}\t{}", c, v));
            }
        }
        for a in &self.attachments {
            line(format!("attach={}\t{}\t{}", a.channel, a.monitor_device_name, a.edge as i32));
        }
        s
    }

    pub fn load(path: &Path, codec: Option<&SecretCodec<'_>>) -> AppConfig {
        match read_lines(path) {
            Some(lines) => AppConfig::parse(&lines, codec),
            None => AppConfig::default(),
        }
    }

    pub fn save(&self, path: &Path, codec: Option<&SecretCodec<'_>>) -> std::io::Result<()> {
        write_utf8_bom(path, &self.serialize(codec))
    }

    /// 평문 형식 문자열에서 읽기(IPC).
    pub fn from_plain_text(text: &str) -> AppConfig {
        let lines: Vec<String> = text.lines().map(|l| l.trim_end_matches('\r').to_string()).collect();
        AppConfig::parse(&lines, None)
    }
}

fn parse_attach(val: &str) -> Option<EdgeAttachment> {
    // 형식: <channel>\t<monitorDeviceName>\t<edge>
    let parts: Vec<&str> = val.split('\t').collect();
    if parts.len() < 3 {
        return None;
    }
    let channel = parts[0].parse::<i32>().ok()?;
    let edge = HandoffEdge::from_i32(parts[2].parse::<i32>().ok()?)?;
    Some(EdgeAttachment {
        channel,
        monitor_device_name: parts[1].to_string(),
        edge,
    })
}
