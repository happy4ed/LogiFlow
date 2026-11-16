//! Swift 판(mac/ 0.15.0) 설정 이어받기 — UserDefaults 도메인 `com.happy4ed.logiflow` 의 두 값.
//!
//! - `edgeEnabled` (Bool) → `edgeSwitchEnabled`
//! - `edgeBindings` (Data, Swift `JSONEncoder` 로 만든 `[EdgeBinding]`) → `attachments`
//!   예: `[{"channel":0,"displayID":69733632,"edge":"right"}]` — channel 0-base · displayID = CGDirectDisplayID ·
//!   edge = left/right/top/bottom. Mac 판 모니터 이름은 displayID 10진 문자열이라 그대로 옮긴다.
//!
//! 읽기(CFPreferences)는 `sys::tray` 가 하고, 여기서는 해석과 적용만 한다(어느 OS 에서나 시험 가능).

use logiflow_core::config::{AppConfig, EdgeAttachment};
use logiflow_core::edge::HandoffEdge;

#[derive(Clone, Debug, PartialEq)]
enum Val {
    Num(f64),
    Str(String),
    Other,
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Option<String> {
        self.ws();
        if self.s.get(self.i) != Some(&b'"') {
            return None;
        }
        self.i += 1;
        let mut out = Vec::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let e = *self.s.get(self.i)?;
                    self.i += 1;
                    match e {
                        b'u' => {
                            let hex = std::str::from_utf8(self.s.get(self.i..self.i + 4)?).ok()?;
                            self.i += 4;
                            let ch = char::from_u32(u32::from_str_radix(hex, 16).ok()?).unwrap_or('?');
                            let mut b = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                        }
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
        None
    }
    /// 값 하나(배열·객체는 건너뛰고 Other).
    fn value(&mut self) -> Option<Val> {
        self.ws();
        match *self.s.get(self.i)? {
            b'"' => self.string().map(Val::Str),
            b'{' | b'[' => {
                let mut depth = 0i32;
                let mut in_str = false;
                while self.i < self.s.len() {
                    let c = self.s[self.i];
                    self.i += 1;
                    if in_str {
                        if c == b'\\' {
                            self.i += 1;
                        } else if c == b'"' {
                            in_str = false;
                        }
                        continue;
                    }
                    match c {
                        b'"' => in_str = true,
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(Val::Other);
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            _ => {
                let start = self.i;
                while self.i < self.s.len() && !matches!(self.s[self.i], b',' | b'}' | b']') && !(self.s[self.i] as char).is_ascii_whitespace() {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.s[start..self.i]).ok()?;
                Some(t.parse::<f64>().map(Val::Num).unwrap_or(Val::Other))
            }
        }
    }
    /// `{ "k": v, ... }` → (키, 값) 목록.
    fn object(&mut self) -> Option<Vec<(String, Val)>> {
        if !self.eat(b'{') {
            return None;
        }
        let mut out = Vec::new();
        if self.eat(b'}') {
            return Some(out);
        }
        loop {
            let k = self.string()?;
            if !self.eat(b':') {
                return None;
            }
            let v = self.value()?;
            out.push((k, v));
            if self.eat(b',') {
                continue;
            }
            if self.eat(b'}') {
                return Some(out);
            }
            return None;
        }
    }
}

fn edge_of(s: &str) -> Option<HandoffEdge> {
    match s {
        "left" => Some(HandoffEdge::Left),
        "right" => Some(HandoffEdge::Right),
        "top" => Some(HandoffEdge::Top),
        "bottom" => Some(HandoffEdge::Bottom),
        _ => None,
    }
}

/// `edgeBindings` JSON → 배치. 해석할 수 없는 항목은 건너뛴다. 같은 화면·변이 겹치면 뒤의 것을 남긴다(Swift `setBinding` 과 같게).
pub fn parse_bindings(json: &[u8]) -> Vec<EdgeAttachment> {
    let mut p = P { s: json, i: 0 };
    let mut out: Vec<EdgeAttachment> = Vec::new();
    if !p.eat(b'[') {
        return out;
    }
    if p.eat(b']') {
        return out;
    }
    while let Some(obj) = p.object() {
        let num = |k: &str| {
            obj.iter().find(|(kk, _)| kk == k).and_then(|(_, v)| match v {
                Val::Num(n) if n.fract() == 0.0 && *n >= 0.0 => Some(*n as u64),
                _ => None,
            })
        };
        let edge = obj.iter().find(|(k, _)| k == "edge").and_then(|(_, v)| match v {
            Val::Str(s) => edge_of(s),
            _ => None,
        });
        if let (Some(ch), Some(id), Some(edge)) = (num("channel"), num("displayID"), edge) {
            if ch < 16 && id <= u32::MAX as u64 {
                let name = id.to_string();
                out.retain(|a| !(a.monitor_device_name == name && a.edge == edge) && a.channel != ch as i32);
                out.push(EdgeAttachment {
                    channel: ch as i32,
                    monitor_device_name: name,
                    edge,
                });
            }
        }
        if p.eat(b',') {
            continue;
        }
        break;
    }
    out.sort_by_key(|a| a.channel);
    out
}

/// 이어받은 값을 새 설정에 넣는다. 값이 없으면(Swift 판을 안 썼으면) 기본값 그대로. 바꾼 것이 있으면 true.
pub fn apply(cfg: &mut AppConfig, edge_enabled: Option<bool>, bindings: Option<&[u8]>) -> bool {
    let mut changed = false;
    if let Some(on) = edge_enabled {
        cfg.edge_switch_enabled = on;
        changed = true;
    }
    if let Some(json) = bindings {
        let list = parse_bindings(json);
        if !list.is_empty() {
            cfg.attachments = list;
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swift_bindings_parse() {
        // Swift JSONEncoder 기본 출력(키 순서는 선언 순, 공백 없음).
        let j = br#"[{"channel":0,"displayID":69733632,"edge":"right"},{"channel":2,"displayID":4,"edge":"top"}]"#;
        let v = parse_bindings(j);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].channel, 0);
        assert_eq!(v[0].monitor_device_name, "69733632");
        assert_eq!(v[0].edge, HandoffEdge::Right);
        assert_eq!(v[1].channel, 2);
        assert_eq!(v[1].edge, HandoffEdge::Top);
    }

    #[test]
    fn swift_bindings_tolerant() {
        // 키 순서 바뀜 · 공백 · 모르는 키 · 잘못된 항목은 건너뜀.
        let j = br#" [ { "edge" : "left", "extra": {"a":[1,2]}, "displayID" : 7, "channel" : 1 },
                       {"channel":"x","displayID":1,"edge":"left"}, {"channel":3,"displayID":9,"edge":"diagonal"} ] "#;
        let v = parse_bindings(j);
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].channel, v[0].monitor_device_name.as_str(), v[0].edge), (1, "7", HandoffEdge::Left));
        assert!(parse_bindings(b"").is_empty());
        assert!(parse_bindings(b"[]").is_empty());
        assert!(parse_bindings(b"{\"channel\":1}").is_empty());
    }

    #[test]
    fn apply_keeps_defaults_without_swift() {
        let mut c = AppConfig::default();
        assert!(!apply(&mut c, None, None));
        assert_eq!(c, AppConfig::default());
        assert!(apply(&mut c, Some(false), Some(br#"[{"channel":1,"displayID":5,"edge":"bottom"}]"#)));
        assert!(!c.edge_switch_enabled);
        assert_eq!(c.attachments.len(), 1);
        assert_eq!(c.attachments[0].edge, HandoffEdge::Bottom);
    }
}
