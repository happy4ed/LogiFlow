//! 리눅스 hidraw — `/sys/class/hidraw/*` 열거와 `/dev/hidrawN` 읽기·쓰기(C# `HidEnumerator`·`HidStream` 이식).
//!
//! hid-logitech-dj(우분투 기본)는 리시버 뒤 기기마다 자식 HID 장치·hidraw 노드를 따로 만든다. 같은 기기에
//! 두 경로로 setHost 가 가지 않게 자식 노드는 건너뛰고 리시버 노드로만 보낸다. 자식의 HID_PHYS 끝 「:N」이
//! 페어링된 기기 index 라, 리시버 노드는 그 index 만 핑하게 힌트를 준다.

use crate::descriptor;
use logiflow_core::hidpp::{HidTransport, OutReport};
use logiflow_core::scan::{HidBackend, HidInterfaceInfo};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const CLASS_ROOT: &str = "/sys/class/hidraw";

fn read_uevent(path: &Path) -> HashMap<String, String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect()
}

/// 「0003:0000046D:0000C548」 → (bus, vid, pid)
fn parse_hid_id(s: &str) -> Option<(u16, u16, u16)> {
    let p: Vec<&str> = s.split(':').collect();
    if p.len() != 3 {
        return None;
    }
    Some((
        u32::from_str_radix(p[0], 16).ok()? as u16,
        u32::from_str_radix(p[1], 16).ok()? as u16,
        u32::from_str_radix(p[2], 16).ok()? as u16,
    ))
}

/// 「0003:046D:C548.0003」 꼴 HID 장치 디렉터리 이름인가.
fn is_hid_dev_name(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 19 && b[4] == b':' && b[9] == b':' && b[14] == b'.' && s.chars().enumerate().all(|(i, c)| matches!(i, 4 | 9 | 14) || c.is_ascii_hexdigit())
}

fn index_hint(hid_dir: &Path) -> Option<Vec<u8>> {
    let mut idx = BTreeSet::new();
    if let Ok(rd) = fs::read_dir(hid_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !is_hid_dev_name(&name) {
                continue;
            }
            let phys = read_uevent(&e.path().join("uevent")).remove("HID_PHYS").unwrap_or_default();
            if let Some((_, n)) = phys.rsplit_once(':') {
                if let Ok(n) = n.parse::<u8>() {
                    if (1..=6).contains(&n) {
                        idx.insert(n);
                    }
                }
            }
        }
    }
    if !idx.is_empty() {
        return Some(idx.into_iter().collect());
    }
    let driver = fs::canonicalize(hid_dir.join("driver"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default();
    if driver == "logitech-hidpp-device" {
        return Some(vec![0xFF]);
    }
    None
}

pub fn enumerate(vendor_filter: u16, usage_page_filter: u16) -> Vec<HidInterfaceInfo> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(CLASS_ROOT) else { return out };
    let mut nodes: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    nodes.sort();
    for node in nodes {
        let name = node.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let Ok(hid_dir) = fs::canonicalize(node.join("device")) else { continue };
        let ue = read_uevent(&hid_dir.join("uevent"));
        let Some((_, vid, pid)) = ue.get("HID_ID").and_then(|s| parse_hid_id(s)) else {
            continue;
        };
        if vendor_filter != 0 && vid != vendor_filter {
            continue;
        }
        // dj 자식(부모 디렉터리가 또 하나의 HID 장치)은 건너뛴다.
        let parent = hid_dir
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if is_hid_dev_name(&parent) {
            continue;
        }
        let Ok(desc) = fs::read(hid_dir.join("report_descriptor")) else { continue };
        let caps = descriptor::parse(&desc);
        if usage_page_filter != 0 && caps.usage_page != usage_page_filter {
            continue;
        }
        out.push(HidInterfaceInfo {
            path: format!("/dev/{}", name),
            vendor_id: vid,
            product_id: pid,
            usage_page: caps.usage_page,
            usage: caps.usage,
            input_report_len: caps.report_len,
            output_report_len: caps.report_len,
            product_string: ue.get("HID_NAME").cloned(),
            caps_read: true,
            device_index_hint: index_hint(&hid_dir),
        });
    }
    out
}

/// 캐시 대조용 — /dev/hidrawN 번호는 재부팅·재연결마다 바뀌므로 HID_PHYS|HID_ID 로 노드를 알아본다.
pub fn identity_of(dev_path: &str) -> Option<String> {
    let name = Path::new(dev_path).file_name()?;
    let hid_dir = fs::canonicalize(Path::new(CLASS_ROOT).join(name).join("device")).ok()?;
    let ue = read_uevent(&hid_dir.join("uevent"));
    Some(format!(
        "{}|{}",
        ue.get("HID_PHYS").cloned().unwrap_or_default(),
        ue.get("HID_ID").cloned().unwrap_or_default()
    ))
}

/// 열기 실패 종류(권한 안내용).
#[derive(Debug, PartialEq, Eq)]
pub enum OpenError {
    Denied,
    Other(i32),
}

pub struct HidrawStream {
    fd: i32,
    input_len: usize,
    output_len: usize,
}

impl HidrawStream {
    pub fn open(info: &HidInterfaceInfo) -> Result<HidrawStream, OpenError> {
        let c = std::ffi::CString::new(info.path.as_str()).map_err(|_| OpenError::Other(libc::EINVAL))?;
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC) };
        if fd < 0 {
            let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            return Err(if e == libc::EACCES || e == libc::EPERM {
                OpenError::Denied
            } else {
                OpenError::Other(e)
            });
        }
        Ok(HidrawStream {
            fd,
            input_len: info.input_report_len,
            output_len: info.output_report_len,
        })
    }

    fn poll(&self, events: i16, timeout_ms: i32) -> bool {
        let mut p = libc::pollfd {
            fd: self.fd,
            events,
            revents: 0,
        };
        unsafe { libc::poll(&mut p, 1, timeout_ms.max(0)) > 0 && (p.revents & events) != 0 }
    }
}

impl Drop for HidrawStream {
    fn drop(&mut self) {
        unsafe {
            if self.fd >= 0 {
                libc::close(self.fd);
            }
        }
    }
}

impl HidTransport for HidrawStream {
    fn output_report_len(&self) -> usize {
        self.output_len
    }

    /// 리포트 ID 를 첫 바이트로 둔 리포트를 write(2) 로 그대로. 코어가 만든 `OutReport` 만 보낸다.
    fn write(&mut self, report: &OutReport, timeout_ms: u32) -> bool {
        let r = report.bytes();
        let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(1) as u64);
        loop {
            let n = unsafe { libc::write(self.fd, r.as_ptr() as *const _, r.len()) };
            if n == r.len() as isize {
                return true;
            }
            let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if n >= 0 || (e != libc::EAGAIN && e != libc::EINTR) {
                return false;
            }
            let left = deadline.saturating_duration_since(Instant::now()).as_millis() as i32;
            if left <= 0 || !self.poll(libc::POLLOUT, left) {
                return false;
            }
        }
    }

    /// poll(2) 로 기다린 뒤 read(2). 번호 붙은 리포트라 첫 바이트가 리포트 ID(Windows ReadFile 과 같은 모양).
    fn read(&mut self, timeout_ms: u32) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; self.input_len.max(64)];
        let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
        loop {
            let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut _, buf.len()) };
            if n > 0 {
                buf.truncate(n as usize);
                return Some(buf);
            }
            if n < 0 {
                let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                if e != libc::EAGAIN && e != libc::EINTR {
                    return None;
                }
            }
            let left = deadline.saturating_duration_since(Instant::now()).as_millis() as i32;
            if left <= 0 || !self.poll(libc::POLLIN, left) {
                return None;
            }
        }
    }
}

pub struct HidrawBackend;

impl HidBackend for HidrawBackend {
    fn enumerate(&self, vendor_filter: u16, usage_page_filter: u16) -> Vec<HidInterfaceInfo> {
        enumerate(vendor_filter, usage_page_filter)
    }
    fn open(&self, info: &HidInterfaceInfo) -> Result<Box<dyn HidTransport>, String> {
        match HidrawStream::open(info) {
            Ok(s) => Ok(Box::new(s)),
            Err(OpenError::Denied) => Err(format!("{} 권한 없음", info.path)),
            Err(OpenError::Other(e)) => Err(format!("{} 열기 실패(errno {})", info.path, e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hid_names_and_ids() {
        assert!(is_hid_dev_name("0003:046D:C548.0003"));
        assert!(!is_hid_dev_name("hidraw3"));
        assert_eq!(parse_hid_id("0003:0000046D:0000C548"), Some((3, 0x046D, 0xC548)));
        assert_eq!(parse_hid_id("bad"), None);
    }
}
