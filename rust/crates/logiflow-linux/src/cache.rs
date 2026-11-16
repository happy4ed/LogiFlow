//! 기기 캐시 — C# 리눅스 CLI 와 같은 형식(탭 구분: 노드 정체(HID_PHYS|HID_ID) · deviceIndex · 0x1814 인덱스 ·
//! 채널 수 · 이름). 나란히 쓰도록 폴더만 따로(`$XDG_CACHE_HOME/logiflow-rs/devices.tsv`).

use logiflow_core::scan::{HidInterfaceInfo, ScanResult};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
    base.join("logiflow-rs")
}

fn file() -> PathBuf {
    dir().join("devices.tsv")
}

pub fn serialize(list: &[ScanResult], identity: &dyn Fn(&str) -> Option<String>) -> String {
    let mut s = String::new();
    for r in list {
        let Some(id) = identity(&r.path) else { continue };
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            id,
            r.device_index,
            r.change_host_index,
            r.host_count,
            r.device_name.replace('\t', " ")
        ));
    }
    s
}

pub fn parse(text: &str, nodes: &[(String, HidInterfaceInfo)]) -> Vec<ScanResult> {
    let by_id: HashMap<&str, &HidInterfaceInfo> = nodes.iter().map(|(id, i)| (id.as_str(), i)).collect();
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            continue;
        }
        let Some(info) = by_id.get(f[0]) else { continue };
        let (Ok(dev), Ok(ch), Ok(hosts)) = (f[1].parse::<u8>(), f[2].parse::<u8>(), f[3].parse::<i32>()) else {
            continue;
        };
        out.push(ScanResult {
            interface: Some((*info).clone()),
            path: info.path.clone(),
            vid_pid: info.vid_pid(),
            device_index: dev,
            change_host_supported: ch != 0,
            change_host_index: ch,
            host_count: hosts,
            device_name: f[4].to_string(),
            connection_type: if dev == 0xFF {
                "직접(USB/BT)".to_string()
            } else {
                format!("리시버 뒤(index {})", dev)
            },
            ..Default::default()
        });
    }
    out
}

pub fn save(list: &[ScanResult], identity: &dyn Fn(&str) -> Option<String>) {
    let _ = std::fs::create_dir_all(dir());
    let _ = std::fs::write(file(), serialize(list, identity));
}

pub fn load(nodes: &[(String, HidInterfaceInfo)]) -> Vec<ScanResult> {
    std::fs::read_to_string(file()).map(|t| parse(&t, nodes)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_matches_csharp_columns() {
        let info = HidInterfaceInfo {
            path: "/dev/hidraw3".into(),
            vendor_id: 0x046D,
            product_id: 0xC548,
            ..Default::default()
        };
        let r = ScanResult {
            path: "/dev/hidraw3".into(),
            device_index: 2,
            change_host_supported: true,
            change_host_index: 0x0A,
            host_count: 3,
            device_name: "MX\tMaster 3S".into(),
            ..Default::default()
        };
        let id = |_: &str| Some("usb-0000:00:14.0-1/input2|0003:0000046D:0000C548".to_string());
        let text = serialize(&[r], &id);
        assert_eq!(text, "usb-0000:00:14.0-1/input2|0003:0000046D:0000C548\t2\t10\t3\tMX Master 3S\n");
        let back = parse(&text, &[("usb-0000:00:14.0-1/input2|0003:0000046D:0000C548".into(), info)]);
        assert_eq!(back.len(), 1);
        assert_eq!((back[0].device_index, back[0].change_host_index, back[0].host_count), (2, 0x0A, 3));
        assert_eq!(back[0].connection_type, "리시버 뒤(index 2)");
    }
}
