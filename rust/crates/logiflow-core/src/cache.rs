//! 기기 캐시(`devices.txt`, 탭 구분 13칸 v1) — C# `DeviceCache` 형식 그대로.
//! 시작 때 캐시로 바로 표시하고 「새로 고침」·기기 변화 때만 재스캔한다. serial 등 민감값은 저장 안 함.

use crate::log::{newline, read_lines, write_utf8_bom};
use crate::scan::{HidInterfaceInfo, ScanResult};
use std::path::Path;

fn escape(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\r', "").replace('\n', " ")
}

fn unescape(s: &str) -> String {
    s.replace("\\t", "\t").replace("\\\\", "\\")
}

fn int(s: &str) -> i32 {
    s.trim().parse::<i32>().unwrap_or(0)
}

pub fn serialize(results: &[ScanResult]) -> String {
    let nl = newline();
    let mut out = String::from("# LogiFlow 기기 캐시 v1");
    out.push_str(nl);
    for r in results {
        let (path, out_len, in_len, page) = match &r.interface {
            Some(i) => (i.path.as_str(), i.output_report_len, i.input_report_len, i.usage_page),
            None => ("", 0, 0, 0),
        };
        let fields = [
            escape(&r.device_name),
            escape(&r.vid_pid),
            escape(&r.connection_type),
            r.device_index.to_string(),
            if r.change_host_supported { "1".into() } else { "0".into() },
            r.change_host_index.to_string(),
            r.host_count.to_string(),
            r.current_host.to_string(),
            escape(path),
            out_len.to_string(),
            in_len.to_string(),
            page.to_string(),
            escape(&r.note),
        ];
        out.push_str(&fields.join("\t"));
        out.push_str(nl);
    }
    out
}

pub fn parse(lines: &[String]) -> Vec<ScanResult> {
    let mut list = Vec::new();
    for raw in lines {
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = raw.split('\t').collect();
        if f.len() < 13 {
            continue;
        }
        let path = unescape(f[8]);
        let interface = if path.is_empty() {
            None
        } else {
            Some(HidInterfaceInfo {
                path: path.clone(),
                output_report_len: int(f[9]).max(0) as usize,
                input_report_len: int(f[10]).max(0) as usize,
                usage_page: int(f[11]) as u16,
                caps_read: true,
                ..Default::default()
            })
        };
        list.push(ScanResult {
            interface,
            device_name: unescape(f[0]),
            vid_pid: unescape(f[1]),
            path,
            connection_type: unescape(f[2]),
            device_index: int(f[3]) as u8,
            change_host_supported: f[4] == "1",
            change_host_index: int(f[5]) as u8,
            host_count: int(f[6]),
            current_host: int(f[7]),
            note: unescape(f[12]),
        });
    }
    list
}

pub fn load(path: &Path) -> Vec<ScanResult> {
    read_lines(path).map(|l| parse(&l)).unwrap_or_default()
}

pub fn save(path: &Path, results: &[ScanResult]) -> std::io::Result<()> {
    write_utf8_bom(path, &serialize(results))
}
