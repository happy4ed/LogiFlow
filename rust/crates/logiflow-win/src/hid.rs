//! Windows HID 전송 — C# `HidEnumerator`·`HidStream` 이식(SetupAPI 열거 · 공유 모드 · 오버랩드 타임아웃).

use logiflow_core::hidpp::{HidTransport, OutReport};
use logiflow_core::scan::{HidBackend, HidInterfaceInfo};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::HumanInterfaceDevice::*;
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::IO::{CancelIo, GetOverlappedResult, OVERLAPPED};

use crate::util::{from_wide, wide};

#[derive(Default, Clone, Copy)]
pub struct WinHidBackend;

/// 경로 예: `\\?\hid#vid_046d&pid_c548&mi_01&col01#...` 에서 vid/pid.
fn extract_hex(lower: &str, key: &str) -> u16 {
    match lower.find(key) {
        Some(i) => {
            let s = i + key.len();
            lower.get(s..s + 4).and_then(|h| u16::from_str_radix(h, 16).ok()).unwrap_or(0)
        }
        None => 0,
    }
}

fn parse_vid_pid(path: &str) -> (u16, u16) {
    let p = path.to_lowercase();
    (extract_hex(&p, "vid_"), extract_hex(&p, "pid_"))
}

fn open_handle(path: &str, access: u32, flags: FILE_FLAGS_AND_ATTRIBUTES) -> windows::core::Result<HANDLE> {
    let w = wide(path);
    unsafe { CreateFileW(PCWSTR(w.as_ptr()), access, FILE_SHARE_READ | FILE_SHARE_WRITE, None, OPEN_EXISTING, flags, None) }
}

fn try_read_interface(path: &str) -> Option<HidInterfaceInfo> {
    // 열거 단계는 접근 권한 0 + 공유 모드 — 다른 프로세스(로지텍 Options 등)와 충돌하지 않게.
    let h = open_handle(path, 0, FILE_FLAGS_AND_ATTRIBUTES(0)).ok()?;
    if h.is_invalid() {
        return None;
    }
    let result = unsafe {
        let mut attrs = HIDD_ATTRIBUTES {
            Size: std::mem::size_of::<HIDD_ATTRIBUTES>() as u32,
            ..Default::default()
        };
        if !HidD_GetAttributes(h, &mut attrs) {
            None
        } else {
            let mut pp = PHIDP_PREPARSED_DATA::default();
            if !HidD_GetPreparsedData(h, &mut pp) {
                None
            } else {
                let mut caps = HIDP_CAPS::default();
                let st = HidP_GetCaps(pp, &mut caps);
                let _ = HidD_FreePreparsedData(pp);
                if st != HIDP_STATUS_SUCCESS {
                    None
                } else {
                    let mut buf = [0u16; 128];
                    let product = if HidD_GetProductString(h, buf.as_mut_ptr() as *mut _, 256) {
                        let s = from_wide(&buf).trim().to_string();
                        Some(s)
                    } else {
                        None
                    };
                    Some(HidInterfaceInfo {
                        path: path.to_string(),
                        vendor_id: attrs.VendorID,
                        product_id: attrs.ProductID,
                        usage_page: caps.UsagePage,
                        usage: caps.Usage,
                        input_report_len: caps.InputReportByteLength as usize,
                        output_report_len: caps.OutputReportByteLength as usize,
                        product_string: product,
                        caps_read: true,
                        device_index_hint: None,
                    })
                }
            }
        }
    };
    unsafe {
        let _ = CloseHandle(h);
    }
    result
}

impl HidBackend for WinHidBackend {
    fn enumerate(&self, vendor_filter: u16, usage_page_filter: u16) -> Vec<HidInterfaceInfo> {
        let mut out = Vec::new();
        unsafe {
            let guid: GUID = HidD_GetHidGuid();
            let set = match SetupDiGetClassDevsW(Some(&guid), PCWSTR::null(), None, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE) {
                Ok(s) => s,
                Err(_) => return out,
            };
            if set.is_invalid() {
                return out;
            }
            let mut idata = SP_DEVICE_INTERFACE_DATA {
                cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            let mut index = 0u32;
            while SetupDiEnumDeviceInterfaces(set, None, &guid, index, &mut idata).is_ok() {
                index += 1;
                let mut required = 0u32;
                let _ = SetupDiGetDeviceInterfaceDetailW(set, &idata, None, 0, Some(&mut required), None);
                if required == 0 {
                    continue;
                }
                // 정렬된 버퍼(u64) 에 상세 구조체.
                let mut buf = vec![0u64; (required as usize).div_ceil(8) + 1];
                let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
                (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
                if SetupDiGetDeviceInterfaceDetailW(set, &idata, Some(detail), required, Some(&mut required), None).is_err() {
                    continue;
                }
                let path_ptr = (detail as *const u8).add(4) as *const u16;
                let max = ((required as usize).saturating_sub(4)) / 2;
                let slice = std::slice::from_raw_parts(path_ptr, max);
                let path = from_wide(slice);
                if path.is_empty() {
                    continue;
                }
                // 경로의 VID/PID 로 값싼 1차 필터(열기 전).
                let (pvid, ppid) = parse_vid_pid(&path);
                if vendor_filter != 0 && pvid != 0 && pvid != vendor_filter {
                    continue;
                }
                // caps/attrs 를 못 읽어도(선점 등) 경로 정보로 남겨 진단에서 보이게.
                let info = try_read_interface(&path).unwrap_or(HidInterfaceInfo {
                    path: path.clone(),
                    vendor_id: pvid,
                    product_id: ppid,
                    caps_read: false,
                    ..Default::default()
                });
                if vendor_filter != 0 && info.vendor_id != vendor_filter && pvid != vendor_filter {
                    continue;
                }
                if usage_page_filter != 0 && info.usage_page != usage_page_filter {
                    continue;
                }
                out.push(info);
            }
            let _ = SetupDiDestroyDeviceInfoList(set);
        }
        out
    }

    fn open(&self, info: &HidInterfaceInfo) -> Result<Box<dyn HidTransport>, String> {
        match open_handle(&info.path, GENERIC_READ.0 | GENERIC_WRITE.0, FILE_FLAG_OVERLAPPED) {
            Ok(h) if !h.is_invalid() => Ok(Box::new(HidStream {
                handle: h,
                input_len: info.input_report_len,
                output_len: info.output_report_len,
            })),
            Ok(_) => Err("HID 인터페이스 열기 실패 (Win32 오류 6)".into()),
            Err(e) => Err(format!("HID 인터페이스 열기 실패 (Win32 오류 {})", (e.code().0 as u32) & 0xFFFF)),
        }
    }
}

/// HID 인터페이스 1개에 대한 오버랩드 읽기/쓰기(타임아웃 지원). 밖으로는 `Box<dyn HidTransport>` 로만 나간다.
pub(crate) struct HidStream {
    handle: HANDLE,
    input_len: usize,
    output_len: usize,
}

impl Drop for HidStream {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

enum Io<'a> {
    Write(&'a [u8]),
    Read(&'a mut [u8]),
}

impl HidStream {
    /// 오버랩드 I/O 1건. 완료 바이트 수, 실패·시간 초과 None.
    /// 시간 초과면 CancelIo 후 완료를 기다려 버퍼 사용이 끝난 뒤에 돌아간다(C# 과 같은 순서).
    fn overlapped(&self, io: Io<'_>, timeout_ms: u32) -> Option<u32> {
        unsafe {
            let ev = CreateEventW(None, true, false, PCWSTR::null()).ok()?;
            let mut ov = OVERLAPPED {
                hEvent: ev,
                ..Default::default()
            };
            let started = match io {
                Io::Write(b) => WriteFile(self.handle, Some(b), None, Some(&mut ov)),
                Io::Read(b) => ReadFile(self.handle, Some(b), None, Some(&mut ov)),
            };
            let mut n = 0u32;
            let result = match started {
                Ok(()) => GetOverlappedResult(self.handle, &ov, &mut n, false).ok().map(|_| n),
                Err(e) if e.code() == ERROR_IO_PENDING.to_hresult() => {
                    if WaitForSingleObject(ev, timeout_ms) != WAIT_OBJECT_0 {
                        let _ = CancelIo(self.handle);
                        let _ = GetOverlappedResult(self.handle, &ov, &mut n, true);
                        None
                    } else if GetOverlappedResult(self.handle, &ov, &mut n, false).is_ok() {
                        Some(n)
                    } else {
                        None
                    }
                }
                Err(_) => None,
            };
            let _ = CloseHandle(ev);
            result
        }
    }
}

impl HidTransport for HidStream {
    fn output_report_len(&self) -> usize {
        self.output_len
    }

    /// 코어가 만든 `OutReport` 만 보낸다 — 이 crate·설정 창은 리포트 내용을 정할 수 없다(구현만 여기).
    fn write(&mut self, report: &OutReport, timeout_ms: u32) -> bool {
        let report = report.bytes();
        let len = if self.output_len > 0 { self.output_len } else { report.len() };
        let mut buf = vec![0u8; len];
        let n = report.len().min(len);
        buf[..n].copy_from_slice(&report[..n]);
        self.overlapped(Io::Write(&buf), timeout_ms).is_some()
    }

    fn read(&mut self, timeout_ms: u32) -> Option<Vec<u8>> {
        let len = if self.input_len > 0 { self.input_len } else { 64 };
        let mut buf = vec![0u8; len];
        let n = self.overlapped(Io::Read(&mut buf), timeout_ms)?;
        if n == 0 {
            return None;
        }
        buf.truncate(n as usize);
        Some(buf)
    }
}
