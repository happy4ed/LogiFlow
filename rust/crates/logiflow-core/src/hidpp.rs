//! HID++ 2.0 프레이밍과 조회 — C# `HidppDevice` 이식.
//!
//! 안전 규칙: 이 모듈 밖으로 공개되는 것은 조회(ping · getFeature · getHostInfo · getDeviceName)뿐이다.
//! 임의 호출(`call`)과 쓰기(`set_host`)는 crate 안에서만 쓰며, 쓰기는 `switcher` 의 가드를 거친 뒤에만 나간다.

use crate::log::DiagLogger;
use std::time::Instant;

pub const REPORT_ID_LONG: u8 = 0x11;
pub const REPORT_ID_SHORT: u8 = 0x10;
/// 응답 매칭용 software id (0 아닌 4비트 값).
const SOFTWARE_ID: u8 = 0x0A;
pub const IROOT_INDEX: u8 = 0x00;
pub const FEATURE_CHANGE_HOST: u16 = 0x1814;
pub const FEATURE_DEVICE_NAME: u16 = 0x0005;

/// 열린 HID 인터페이스 1개(OS 어댑터가 구현). report[0] 은 리포트 ID.
pub trait HidTransport {
    /// 출력 리포트 길이(바이트, 리포트 ID 포함). 0 이면 모름.
    fn output_report_len(&self) -> usize;
    /// 출력 리포트 전송. 시간 초과·실패 시 false.
    fn write(&mut self, report: &[u8], timeout_ms: u32) -> bool;
    /// 입력 리포트 1건 수신. 시간 초과 시 None.
    fn read(&mut self, timeout_ms: u32) -> Option<Vec<u8>>;
}

/// HID++ 장치가 반환한 오류 응답.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidppError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for HidppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub fn describe_error(code: i32) -> String {
    match code {
        0 => "NoError".into(),
        1 => "Unknown".into(),
        2 => "InvalidArgument".into(),
        3 => "OutOfRange".into(),
        4 => "HardwareError".into(),
        5 => "LogitechInternal".into(),
        6 => "InvalidFeatureIndex".into(),
        7 => "InvalidFunctionId".into(),
        8 => "Busy".into(),
        9 => "Unsupported".into(),
        _ => format!("Error(0x{:02X})", code),
    }
}

fn func_byte(func_index: u8) -> u8 {
    ((func_index & 0x0F) << 4) | SOFTWARE_ID
}

/// 출력 리포트 길이로 long(0x11·20바이트)/short(0x10·7바이트) 프레임을 만든다.
pub(crate) fn build_report(output_len: usize, device_index: u8, feature_index: u8, func_index: u8, params: &[u8]) -> Vec<u8> {
    let use_long = output_len >= 20;
    let report_len = if use_long { 20 } else { 7 };
    let max_params = if use_long { 16 } else { 3 };
    let mut report = vec![0u8; report_len];
    report[0] = if use_long { REPORT_ID_LONG } else { REPORT_ID_SHORT };
    report[1] = device_index;
    report[2] = feature_index;
    report[3] = func_byte(func_index);
    let n = params.len().min(max_params);
    report[4..4 + n].copy_from_slice(&params[..n]);
    report
}

/// 한 HID++ 인터페이스 위에서 특정 deviceIndex 를 상대로 호출한다.
pub struct HidppDevice<'a> {
    t: &'a mut dyn HidTransport,
    log: &'a DiagLogger,
}

impl<'a> HidppDevice<'a> {
    pub fn new(t: &'a mut dyn HidTransport, log: &'a DiagLogger) -> Self {
        HidppDevice { t, log }
    }

    /// HID++ 호출. Ok(None) = 시간 초과, Err = 오류 응답.
    pub(crate) fn call(&mut self, device_index: u8, feature_index: u8, func_index: u8, params: &[u8], timeout_ms: u32) -> Result<Option<[u8; 16]>, HidppError> {
        let fb = func_byte(func_index);
        let report = build_report(self.t.output_report_len(), device_index, feature_index, func_index, params);
        self.log.tx(device_index, &report);

        if !self.t.write(&report, timeout_ms) {
            self.log
                .timeout(&format!("Write dev=0x{:02X} feat=0x{:02X} func={}", device_index, feature_index, func_index));
            return Ok(None);
        }

        let start = Instant::now();
        loop {
            let elapsed = start.elapsed().as_millis() as i64;
            let remaining = timeout_ms as i64 - elapsed;
            if remaining <= 0 {
                self.log
                    .timeout(&format!("Read dev=0x{:02X} feat=0x{:02X} func={}", device_index, feature_index, func_index));
                return Ok(None);
            }
            let resp = match self.t.read(remaining as u32) {
                Some(r) => r,
                None => {
                    self.log
                        .timeout(&format!("Read dev=0x{:02X} feat=0x{:02X} func={}", device_index, feature_index, func_index));
                    return Ok(None);
                }
            };
            self.log.rx(&resp);
            if resp.len() < 4 {
                continue;
            }
            let (r_id, r_dev, r_feat, r_func) = (resp[0], resp[1], resp[2], resp[3]);

            // HID++ 2.0 오류 응답: feature index 자리에 0xFF.
            if (r_id == REPORT_ID_LONG || r_id == REPORT_ID_SHORT)
                && r_feat == 0xFF
                && r_dev == device_index
                && resp.len() >= 6
                && resp[3] == feature_index
                && resp[4] == fb
            {
                let code = resp[5] as i32;
                return Err(HidppError {
                    code,
                    message: format!("HID++2.0 {}", describe_error(code)),
                });
            }
            // HID++ 1.0 오류 응답(리시버): 0x8F.
            if r_id == REPORT_ID_SHORT && r_feat == 0x8F && r_dev == device_index {
                let code = if resp.len() >= 6 { resp[5] as i32 } else { -1 };
                let shown = if code < 0 { "FFFFFFFF".to_string() } else { format!("{:02X}", code) };
                return Err(HidppError {
                    code,
                    message: format!("HID++1.0 error 0x{}", shown),
                });
            }
            // 정상 응답 매칭.
            if r_dev == device_index && r_feat == feature_index && r_func == fb {
                let mut payload = [0u8; 16];
                let copy = 16.min(resp.len() - 4);
                payload[..copy].copy_from_slice(&resp[4..4 + copy]);
                return Ok(Some(payload));
            }
            // 그 밖은 비동기 알림 — 무시하고 계속 읽는다.
        }
    }

    /// IRoot ping(func1): 응답 echo 로 장치 존재 확인. 오류 응답도 "응답했다"는 증거.
    pub fn ping(&mut self, device_index: u8, timeout_ms: u32) -> bool {
        const MARKER: u8 = 0x5A;
        match self.call(device_index, IROOT_INDEX, 1, &[0x00, 0x00, MARKER], timeout_ms) {
            Err(_) => true,
            Ok(None) => false,
            Ok(Some(p)) => p[2] == MARKER,
        }
    }

    /// IRoot getFeature(func0): featureId 의 런타임 인덱스. 0 이면 미지원(또는 무응답).
    pub fn get_feature_index(&mut self, device_index: u8, feature_id: u16, timeout_ms: u32) -> Result<u8, HidppError> {
        let params = [(feature_id >> 8) as u8, (feature_id & 0xFF) as u8];
        Ok(self.call(device_index, IROOT_INDEX, 0, &params, timeout_ms)?.map(|p| p[0]).unwrap_or(0))
    }

    /// 0x1814 func0 getHostInfo → (호스트 개수, 현재 호스트 0-base). 무응답 None.
    pub fn get_host_info(&mut self, device_index: u8, change_host_index: u8, timeout_ms: u32) -> Result<Option<(i32, i32)>, HidppError> {
        Ok(self
            .call(device_index, change_host_index, 0, &[], timeout_ms)?
            .map(|p| (p[0] as i32, p[1] as i32)))
    }

    /// 0x1814 func1 setHost — 호스트 전환(쓰기). crate 안 `switcher` 에서만 호출한다.
    /// 기기가 전환 즉시 연결을 리셋하므로 응답을 기다리지 않는다. drain_ms=0 이면 바로 돌아온다(묶음 전송).
    pub(crate) fn set_host(&mut self, device_index: u8, change_host_index: u8, target_host: u8, write_timeout_ms: u32, drain_ms: u32) -> bool {
        let report = build_report(self.t.output_report_len(), device_index, change_host_index, 1, &[target_host]);
        self.log.log(&format!(
            "TX(setHost) dev=0x{:02X} feat=0x{:02X} target={}  [{}]",
            device_index,
            change_host_index,
            target_host,
            DiagLogger::hex(&report)
        ));
        if !self.t.write(&report, write_timeout_ms) {
            self.log.error(&format!("setHost 전송 실패 dev=0x{:02X}", device_index));
            return false;
        }
        if drain_ms == 0 {
            return true;
        }
        if let Some(resp) = self.t.read(drain_ms) {
            self.log.rx(&resp);
        }
        true
    }

    /// 0x0005 로 기기 이름을 읽는다(best-effort). 실패 시 None.
    pub fn get_device_name(&mut self, device_index: u8, name_feature_index: u8, timeout_ms: u32) -> Option<String> {
        let head = self.call(device_index, name_feature_index, 0, &[], timeout_ms).ok()??;
        let count = head[0] as usize;
        if count == 0 || count > 64 {
            return None;
        }
        let mut name = String::with_capacity(count);
        let mut idx = 0usize;
        let mut guard = 0;
        while idx < count && guard < 16 {
            guard += 1;
            let chunk = match self.call(device_index, name_feature_index, 1, &[idx as u8], timeout_ms) {
                Ok(Some(c)) => c,
                Ok(None) => break,
                Err(_) => return None,
            };
            let mut added = 0;
            for &b in chunk.iter() {
                if idx >= count {
                    break;
                }
                if b == 0 {
                    break;
                }
                if (0x20..0x7F).contains(&b) {
                    name.push(b as char);
                }
                idx += 1;
                added += 1;
            }
            if added == 0 {
                break;
            }
        }
        let name = name.trim().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }
}
