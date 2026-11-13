//! 통제된 쓰기 — C# `HostSwitcher` 이식. 0x1814 func1 setHost 로 호스트(채널)를 전환한다.
//!
//! 가드: (a) 0x1814 지원 확인된 기기 (b) target 이 0~hostCount-1 범위 (c) 명시 트리거에서만 호출
//! (가장자리·트레이·버튼 — 호출 측 책임). 이 모듈 밖에서는 어떤 쓰기도 하지 않는다.

use crate::hidpp::HidppDevice;
use crate::log::DiagLogger;
use crate::scan::{HidBackend, ScanResult};
use std::time::Duration;

const WRITE_TIMEOUT_MS: u32 = 400;

/// (a)(b) 가드. 위반 사유(로그·안내 문구용) 또는 None.
fn guard(d: &ScanResult, target: i32) -> Option<GuardFail> {
    if !d.change_host_supported || d.change_host_index == 0 {
        return Some(GuardFail::Unsupported);
    }
    if target < 0 || (d.host_count >= 0 && target >= d.host_count) {
        return Some(GuardFail::OutOfRange);
    }
    None
}

enum GuardFail {
    Unsupported,
    OutOfRange,
}

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// 단일 기기 전환. 가드 위반 시 false(아무것도 전송 안 함).
pub fn switch_one(backend: &dyn HidBackend, device: &ScanResult, target: i32, log: &DiagLogger) -> bool {
    let iface = match &device.interface {
        Some(i) => i,
        None => return false,
    };
    match guard(device, target) {
        Some(GuardFail::Unsupported) => {
            log.error(&format!("전환 거부(0x1814 미지원): {}", device.device_name));
            return false;
        }
        Some(GuardFail::OutOfRange) => {
            log.error(&format!(
                "전환 거부(범위 밖 채널 {}, 호스트 {}): {}",
                target, device.host_count, device.device_name
            ));
            return false;
        }
        None => {}
    }
    let mut stream = match backend.open(iface) {
        Ok(s) => s,
        Err(e) => {
            log.error(&format!("전환 예외: {}: {}", device.device_name, e));
            return false;
        }
    };
    log.log(&format!(
        "전환 시도: {} (dev=0x{:02X}, 0x1814 index=0x{:02X}) → 채널 {}",
        device.device_name,
        device.device_index,
        device.change_host_index,
        target + 1
    ));
    let mut h = HidppDevice::new(stream.as_mut(), log);
    h.set_host(device.device_index, device.change_host_index, target as u8, WRITE_TIMEOUT_MS, 120)
}

/// 전환 가능한 모든 기기를 같은 채널로 함께 전환한다(재시도 없음).
pub fn switch_all(backend: &dyn HidBackend, devices: &[ScanResult], target: i32, log: &DiagLogger) -> usize {
    log.log(&format!("=== 전체 전환 시작 → 채널 {} ===", target + 1));
    let mut sent = 0;
    for d in devices {
        if !d.change_host_supported {
            continue;
        }
        if switch_one(backend, d, target, log) {
            sent += 1;
        }
    }
    log.log(&format!("=== 전체 전환 종료: {}개 전송 ===", sent));
    sent
}

/// 로그온 워커 전용 — 기기별로 깨우기(ping) → setHost → 확인, 최대 attempts 회.
/// 성공 판정: 쓰기 성공 + (응답 사라짐 || 현재채널 == 목표).
pub fn switch_all_verbose(backend: &dyn HidBackend, devices: &[ScanResult], target: i32, raw: &DiagLogger, say: &mut dyn FnMut(&str), attempts: u32) -> usize {
    let mut switched = 0;
    for d in devices {
        let iface = match &d.interface {
            Some(i) => i,
            None => continue,
        };
        match guard(d, target) {
            Some(GuardFail::Unsupported) => {
                say(&format!("  {}: 0x1814 미지원 — 건너뜀", d.device_name));
                continue;
            }
            Some(GuardFail::OutOfRange) => {
                say(&format!("  {}: 채널 {} 범위 밖(호스트 {}) — 건너뜀", d.device_name, target + 1, d.host_count));
                continue;
            }
            None => {}
        }
        let mut ok = false;
        let mut a = 1;
        while a <= attempts && !ok {
            match backend.open(iface) {
                Ok(mut stream) => {
                    let mut h = HidppDevice::new(stream.as_mut(), raw);
                    // 명령 파이프 깨우기 — 콜드부팅 직후 첫 명령이 무시되는 것 완화.
                    let _ = h.ping(d.device_index, 200);
                    let wrote = h.set_host(d.device_index, d.change_host_index, target as u8, WRITE_TIMEOUT_MS, 120);
                    let info = h.get_host_info(d.device_index, d.change_host_index, 300);
                    match info {
                        Err(e) => say(&format!("  {}: 시도{} 예외 {}", d.device_name, a, e.message)),
                        Ok(got) => {
                            // 전환되면 기기가 이 리시버에서 사라져 응답이 없는 게 정상(= 성공 신호).
                            if wrote && got.map(|(_, cur)| cur == target).unwrap_or(true) {
                                ok = true;
                            }
                            say(&format!(
                                "  {}: 시도{} 쓰기={} 확인={} → {}",
                                d.device_name,
                                a,
                                if wrote { "성공" } else { "실패" },
                                match got {
                                    Some((_, cur)) => format!("현재채널 {}", cur + 1),
                                    None => "응답없음(전환추정)".to_string(),
                                },
                                if ok { "전환됨" } else { "미전환" }
                            ));
                        }
                    }
                }
                Err(e) => say(&format!("  {}: 시도{} 예외 {}", d.device_name, a, e)),
            }
            if !ok {
                sleep_ms(150);
            }
            a += 1;
        }
        if ok {
            switched += 1;
        }
    }
    switched
}

/// 바탕화면 워커 전용 — 모든 기기에 setHost 를 대기 없이 먼저 연달아 쓰고, 그다음 확인한다.
/// 첫 쓰기 성공 순간 on_delivered(glow 최고점). 실패 기기만 ping 으로 깨운 뒤 재시도.
pub fn switch_all_burst(
    backend: &dyn HidBackend,
    devices: &[ScanResult],
    target: i32,
    raw: &DiagLogger,
    say: &mut dyn FnMut(&str),
    attempts: u32,
    on_delivered: Option<&dyn Fn()>,
) -> usize {
    let mut pending: Vec<&ScanResult> = Vec::new();
    for d in devices {
        if d.interface.is_none() {
            continue;
        }
        match guard(d, target) {
            Some(GuardFail::Unsupported) => {
                say(&format!("  {}: 0x1814 미지원 — 건너뜀", d.device_name));
                continue;
            }
            Some(GuardFail::OutOfRange) => {
                say(&format!("  {}: 채널 {} 범위 밖(호스트 {}) — 건너뜀", d.device_name, target + 1, d.host_count));
                continue;
            }
            None => pending.push(d),
        }
    }

    let mut switched = 0;
    let mut delivered = false;
    let mut a = 1;
    while a <= attempts && !pending.is_empty() {
        let mut streams: Vec<Option<Box<dyn crate::hidpp::HidTransport>>> = Vec::with_capacity(pending.len());
        let mut wrote = vec![false; pending.len()];
        // 1) 쓰기 묶음.
        for (i, d) in pending.iter().enumerate() {
            match backend.open(d.interface.as_ref().unwrap()) {
                Ok(mut s) => {
                    {
                        let mut h = HidppDevice::new(s.as_mut(), raw);
                        if a > 1 {
                            let _ = h.ping(d.device_index, 200);
                        }
                        wrote[i] = h.set_host(d.device_index, d.change_host_index, target as u8, WRITE_TIMEOUT_MS, 0);
                    }
                    streams.push(Some(s));
                }
                Err(e) => {
                    say(&format!("  {}: 시도{} 쓰기 예외 {}", d.device_name, a, e));
                    streams.push(None);
                }
            }
            if wrote[i] && !delivered {
                delivered = true;
                if let Some(cb) = on_delivered {
                    cb();
                }
            }
        }
        // 2) 확인 — 전환되면 기기가 사라져 응답이 없는 게 정상(= 성공 신호).
        let mut next = Vec::new();
        for (i, d) in pending.iter().enumerate() {
            let mut ok = false;
            let mut got: Option<(i32, i32)> = None;
            let mut why: Option<String> = None;
            if wrote[i] {
                if let Some(s) = streams[i].as_mut() {
                    let mut h = HidppDevice::new(s.as_mut(), raw);
                    match h.get_host_info(d.device_index, d.change_host_index, 300) {
                        Ok(g) => {
                            got = g;
                            ok = g.map(|(_, cur)| cur == target).unwrap_or(true);
                        }
                        Err(e) => why = Some(format!("확인 예외 {}", e.message)),
                    }
                }
            }
            let confirm = match (&why, wrote[i], got) {
                (Some(w), _, _) => w.clone(),
                (None, false, _) => "-".to_string(),
                (None, true, Some((_, cur))) => format!("현재채널 {}", cur + 1),
                (None, true, None) => "응답없음(전환추정)".to_string(),
            };
            say(&format!(
                "  {}: 시도{} 쓰기={} 확인={} → {}",
                d.device_name,
                a,
                if wrote[i] { "성공" } else { "실패" },
                confirm,
                if ok { "전환됨" } else { "미전환" }
            ));
            if ok {
                switched += 1;
            } else {
                next.push(*d);
            }
        }
        drop(streams);
        pending = next;
        if !pending.is_empty() && a < attempts {
            sleep_ms(150);
        }
        a += 1;
    }
    switched
}
