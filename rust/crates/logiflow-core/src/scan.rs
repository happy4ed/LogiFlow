//! 로지텍 HID++ 장치 열거·조회(읽기 전용) — C# `DeviceScanner`·`ScanResult`·`HidInterfaceInfo` 이식.

use crate::hidpp::{HidTransport, HidppDevice, FEATURE_CHANGE_HOST, FEATURE_DEVICE_NAME};
use crate::log::DiagLogger;

pub const LOGITECH_VID: u16 = 0x046D;
pub const HIDPP_USAGE_PAGE: u16 = 0xFF43;
pub const USAGE_LONG: u16 = 0x0202;
pub const USAGE_SHORT: u16 = 0x0201;

/// deviceIndex 후보: 직접 연결(0xFF) + 리시버 뒤 1~6.
pub const DEVICE_INDEX_CANDIDATES: [u8; 7] = [0xFF, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
const PROBE_TIMEOUT_MS: u32 = 600;
const QUERY_TIMEOUT_MS: u32 = 800;

/// 열거된 HID 인터페이스 1개의 정적 속성.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HidInterfaceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub input_report_len: usize,
    pub output_report_len: usize,
    pub product_string: Option<String>,
    /// caps/attrs 를 실제로 읽었는지(false 면 경로에서만 추출).
    pub caps_read: bool,
    /// 핑할 deviceIndex 만 알 때(리눅스 hidraw). None 이면 0xFF+1~6 전부.
    pub device_index_hint: Option<Vec<u8>>,
}

impl HidInterfaceInfo {
    pub fn vid_pid(&self) -> String {
        format!("{:04X}:{:04X}", self.vendor_id, self.product_id)
    }
}

/// 발견된 HID++ 장치 1개(= 인터페이스 × deviceIndex)의 진단 결과.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanResult {
    pub interface: Option<HidInterfaceInfo>,
    pub device_name: String,
    pub vid_pid: String,
    pub path: String,
    pub connection_type: String,
    pub device_index: u8,
    pub change_host_supported: bool,
    pub change_host_index: u8,
    pub host_count: i32,
    pub current_host: i32,
    pub note: String,
}

impl Default for ScanResult {
    fn default() -> Self {
        ScanResult {
            interface: None,
            device_name: String::new(),
            vid_pid: String::new(),
            path: String::new(),
            connection_type: String::new(),
            device_index: 0,
            change_host_supported: false,
            change_host_index: 0,
            host_count: -1,
            current_host: -1,
            note: String::new(),
        }
    }
}

/// OS 어댑터 — 열거와 열기. 열기 실패 문자열은 C# 예외 메시지와 같은 모양으로.
pub trait HidBackend: Sync {
    fn enumerate(&self, vendor_filter: u16, usage_page_filter: u16) -> Vec<HidInterfaceInfo>;
    fn open(&self, info: &HidInterfaceInfo) -> Result<Box<dyn HidTransport>, String>;
}

/// 연결된 로지텍 HID 인터페이스 경로의 지문(변화 감지용·HID++ I/O 없음).
pub fn signature(backend: &dyn HidBackend) -> String {
    let mut paths: Vec<String> = backend.enumerate(LOGITECH_VID, 0).into_iter().map(|i| i.path).collect();
    paths.sort();
    paths.join("|")
}

/// 전환 가능(0x1814 지원 + 호스트 개수 있음) 기기만.
pub fn switchable(results: &[ScanResult]) -> Vec<ScanResult> {
    results.iter().filter(|r| r.change_host_supported && r.host_count > 0).cloned().collect()
}

pub fn scan(backend: &dyn HidBackend, log: &DiagLogger) -> Vec<ScanResult> {
    let mut results = Vec::new();
    log.log("=== 스캔 시작 ===");
    // usagePage 필터 없이 VID 046D 인터페이스를 전부 가져와 진단에 남긴다.
    let interfaces = backend.enumerate(LOGITECH_VID, 0);
    log.log(&format!("로지텍(VID 046D) HID 인터페이스 {}개 발견", interfaces.len()));
    for i in &interfaces {
        log.log(&format!(
            "  iface {} usagePage=0x{:04X} usage=0x{:04X} in={} out={} caps={} path={}",
            i.vid_pid(),
            i.usage_page,
            i.usage,
            i.input_report_len,
            i.output_report_len,
            if i.caps_read { "ok" } else { "unread" },
            i.path
        ));
    }
    for i in &interfaces {
        // HID++ 후보: long(0x0202)·short(0x0201) 벤더 컬렉션, 그리고 벤더 usagePage(0xFFxx)에
        // 7바이트 이상 출력 리포트가 있는 인터페이스. caps 를 못 읽은 것은 건너뛰되 로그엔 남는다.
        let candidate = i.caps_read && (i.usage == USAGE_LONG || i.usage == USAGE_SHORT || (i.usage_page >= 0xFF00 && i.output_report_len >= 7));
        log.log(&format!(
            "  iface {} usage=0x{:04X} 후보={}",
            i.vid_pid(),
            i.usage,
            if candidate { "예" } else { "아니오" }
        ));
        if candidate {
            probe_interface(backend, i, log, &mut results);
        }
    }
    let deduped = deduplicate(&results);
    if deduped.is_empty() {
        log.log("응답한 HID++ 장치 없음");
    }
    log.log(&format!(
        "=== 스캔 종료: 후보 응답 {}건 → 중복 제거 후 장치 {}개 ===",
        results.len(),
        deduped.len()
    ));
    deduped
}

/// 같은 물리 기기가 여러 인터페이스로 잡히면 하나로 합친다. 키 = 이름 + deviceIndex + 연결 방식.
/// 0x1814 정보가 있는 쪽을 유지한다.
pub fn deduplicate(results: &[ScanResult]) -> Vec<ScanResult> {
    let mut list: Vec<(String, ScanResult)> = Vec::new();
    for r in results {
        let key = format!("{}|{}|{}", r.device_name, r.device_index, r.connection_type);
        if let Some(pos) = list.iter().position(|(k, _)| *k == key) {
            if !list[pos].1.change_host_supported && r.change_host_supported {
                list[pos].1 = r.clone();
            }
        } else {
            list.push((key, r.clone()));
        }
    }
    list.into_iter().map(|(_, r)| r).collect()
}

fn probe_interface(backend: &dyn HidBackend, iface: &HidInterfaceInfo, log: &DiagLogger, results: &mut Vec<ScanResult>) {
    let mut stream = match backend.open(iface) {
        Ok(s) => s,
        Err(e) => {
            log.error(&format!("인터페이스 열기 실패 {}: {}", iface.path, e));
            return;
        }
    };
    let candidates: Vec<u8> = iface.device_index_hint.clone().unwrap_or_else(|| DEVICE_INDEX_CANDIDATES.to_vec());
    for device_index in candidates {
        let present = {
            let mut h = HidppDevice::new(stream.as_mut(), log);
            // 잠자는 기기(특히 키보드)를 고려해 1회 재시도.
            h.ping(device_index, PROBE_TIMEOUT_MS) || h.ping(device_index, PROBE_TIMEOUT_MS)
        };
        log.log(&format!(
            "  ping {} dev=0x{:02X} → {}",
            iface.vid_pid(),
            device_index,
            if present { "응답" } else { "타임아웃" }
        ));
        if !present {
            continue;
        }
        let mut h = HidppDevice::new(stream.as_mut(), log);
        results.push(build_result(iface, &mut h, device_index, log));
    }
}

fn append_note(existing: &str, add: &str) -> String {
    if existing.is_empty() {
        add.to_string()
    } else {
        format!("{}; {}", existing, add)
    }
}

fn build_result(iface: &HidInterfaceInfo, h: &mut HidppDevice<'_>, device_index: u8, log: &DiagLogger) -> ScanResult {
    let mut r = ScanResult {
        interface: Some(iface.clone()),
        vid_pid: iface.vid_pid(),
        path: iface.path.clone(),
        device_index,
        connection_type: if device_index == 0xFF {
            "직접(USB/BT)".to_string()
        } else {
            format!("리시버 뒤(index {})", device_index)
        },
        ..Default::default()
    };

    // 0x1814 Change Host 인덱스 런타임 조회.
    match h.get_feature_index(device_index, FEATURE_CHANGE_HOST, QUERY_TIMEOUT_MS) {
        Ok(ch) if ch != 0 => {
            r.change_host_supported = true;
            r.change_host_index = ch;
            match h.get_host_info(device_index, ch, QUERY_TIMEOUT_MS) {
                Ok(Some((nb, cur))) => {
                    r.host_count = nb;
                    r.current_host = cur;
                }
                Ok(None) => r.note = append_note(&r.note, "getHostInfo 무응답"),
                Err(e) => r.note = append_note(&r.note, &format!("0x1814 조회 오류: {}", e.message)),
            }
        }
        Ok(_) => {
            r.change_host_supported = false;
            r.note = append_note(&r.note, "0x1814 미지원");
        }
        Err(e) => r.note = append_note(&r.note, &format!("0x1814 조회 오류: {}", e.message)),
    }

    // 기기 이름: HID++ 0x0005 우선, 실패 시 OS 제품 문자열.
    match h.get_feature_index(device_index, FEATURE_DEVICE_NAME, QUERY_TIMEOUT_MS) {
        Ok(ni) if ni != 0 => {
            if let Some(n) = h.get_device_name(device_index, ni, QUERY_TIMEOUT_MS) {
                r.device_name = n;
            }
        }
        Ok(_) => {}
        Err(e) => log.error(&format!("0x0005 조회 오류 dev=0x{:02X}: {}", device_index, e.message)),
    }
    if r.device_name.is_empty() {
        r.device_name = match &iface.product_string {
            Some(p) if !p.is_empty() => p.clone(),
            _ => "(이름 미상)".to_string(),
        };
    }
    r
}

/// 기기 이름으로 키보드 아이콘을 쓸지(설정 창 행 아이콘) — C# MainForm 휴리스틱.
pub fn looks_like_keyboard(name: &str) -> bool {
    let l = name.to_lowercase();
    l.contains("keyboard") || l.contains("mechanical") || l.contains("keys")
}
