//! IOKit HID 전송 — Swift 판 `HidManager` · `HidDevice` · `HidRunLoop` 이식(코어 `HidBackend` · `HidTransport`).
//!
//! - 열거: `IOHIDManagerCopyDevices`(VID 046D 매칭). 한 물리 기기의 컬렉션(long 0x0202 · short 0x0201)이 각각 하나.
//! - 리포트 길이: macOS `Max*ReportSize` 는 ID 바이트 제외 → Windows 의미(ID 포함)로 +1 해 둔다(코어의 long/short 선택이 그대로).
//! - 쓰기: `IOHIDDeviceSetReport(output, reportID = report[0], 버퍼 전체)` — hidapi 와 같은 방식.
//! - 읽기: 입력 리포트 콜백(전용 CFRunLoop 스레드)이 큐에 쌓고 `read` 가 시간 제한으로 기다린다. 콜백 버퍼가 ID 를
//!   포함하는지 기기·OS 판마다 달라, 첫 바이트가 0x10/0x11 이 아니면 콜백의 reportID 를 앞에 붙인다(Swift 판과 같게).
//! - 열기는 공유 모드(seize 아님) — Logi Options+ 와 함께 써도 충돌하지 않게.
//! - 닫기는 콜백이 도는 그 스레드에서(콜백 해제 · 런루프에서 빼기 · 닫기) 끝낸 뒤 버퍼를 놓는다 — 도는 콜백과 겹치지 않게.

use super::ffi::*;
use core_foundation_sys::base::{kCFAllocatorDefault, CFIndex, CFRelease, CFRetain, CFTypeRef};
use core_foundation_sys::dictionary::{kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks, CFDictionaryCreate};
use core_foundation_sys::number::{kCFNumberSInt32Type, CFNumberCreate};
use core_foundation_sys::runloop::{
    kCFRunLoopDefaultMode, CFRunLoopAddTimer, CFRunLoopGetCurrent, CFRunLoopRef, CFRunLoopRun, CFRunLoopTimerCreate, CFRunLoopTimerRef, CFRunLoopWakeUp,
};
use core_foundation_sys::set::{CFSetGetCount, CFSetGetValues};
use logiflow_core::hidpp::{HidTransport, OutReport};
use logiflow_core::scan::{HidBackend, HidInterfaceInfo};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::{mpsc, Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

const REPORT_ID_SHORT: u8 = 0x10;
const REPORT_ID_LONG: u8 = 0x11;

/// 입력 콜백 전용 런루프 스레드(앱 수명 동안 하나).
struct HidRunLoop {
    rl: usize,
}

extern "C" fn keepalive(_t: CFRunLoopTimerRef, _i: *mut c_void) {}

fn run_loop() -> CFRunLoopRef {
    static RL: OnceLock<HidRunLoop> = OnceLock::new();
    RL.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<usize>();
        let _ = std::thread::Builder::new().name("LogiFlow.HidRunLoop".into()).spawn(move || unsafe {
            let rl = CFRunLoopGetCurrent();
            // 소스가 없으면 런루프가 바로 끝나므로 깨어 있게 긴 주기 타이머를 하나 건다.
            let t = CFRunLoopTimerCreate(
                kCFAllocatorDefault,
                core_foundation_sys::date::CFAbsoluteTimeGetCurrent() + 3600.0,
                3600.0,
                0,
                0,
                keepalive,
                std::ptr::null_mut(),
            );
            CFRunLoopAddTimer(rl, t, kCFRunLoopDefaultMode);
            let _ = tx.send(rl as usize);
            loop {
                CFRunLoopRun();
            }
        });
        HidRunLoop {
            rl: rx.recv_timeout(Duration::from_secs(5)).unwrap_or(0),
        }
    })
    .rl as CFRunLoopRef
}

/// 런루프 스레드에서 f 를 실행하고 끝날 때까지 기다린다(최대 2초).
fn on_run_loop(f: impl FnOnce() + Send + 'static) {
    let rl = run_loop();
    if rl.is_null() {
        f();
        return;
    }
    let (tx, rx) = mpsc::channel::<()>();
    let job: Mutex<Option<Box<dyn FnOnce() + Send>>> = Mutex::new(Some(Box::new(f)));
    let block = block2::RcBlock::new(move || {
        if let Some(j) = job.lock().ok().and_then(|mut g| g.take()) {
            j();
        }
        let _ = tx.send(());
    });
    unsafe {
        CFRunLoopPerformBlock(rl, kCFRunLoopDefaultMode as CFTypeRef, &block);
        CFRunLoopWakeUp(rl);
    }
    let _ = rx.recv_timeout(Duration::from_secs(2));
}

fn prop(dev: IOHIDDeviceRef, key: &str) -> CFTypeRef {
    let k = cfstr(key);
    unsafe { IOHIDDeviceGetProperty(dev, k.0 as _) }
}

fn int_prop(dev: IOHIDDeviceRef, key: &str) -> Option<i64> {
    cf_i64(prop(dev, key))
}

fn read_info(dev: IOHIDDeviceRef) -> HidInterfaceInfo {
    let vendor_id = int_prop(dev, "VendorID").unwrap_or(0) as u16;
    let product_id = int_prop(dev, "ProductID").unwrap_or(0) as u16;
    let usage_page = int_prop(dev, "PrimaryUsagePage").unwrap_or(0) as u16;
    let usage = int_prop(dev, "PrimaryUsage").unwrap_or(0) as u16;
    let input_report_len = int_prop(dev, "MaxInputReportSize").filter(|v| *v > 0).map(|v| v as usize + 1).unwrap_or(0);
    let output_report_len = int_prop(dev, "MaxOutputReportSize").filter(|v| *v > 0).map(|v| v as usize + 1).unwrap_or(0);
    let product_string = cf_string(prop(dev, "Product"));
    let loc = int_prop(dev, "LocationID").unwrap_or(0) as u32;
    let mut entry: u64 = 0;
    unsafe {
        let svc = IOHIDDeviceGetService(dev);
        if svc != 0 {
            let _ = IORegistryEntryGetRegistryEntryID(svc, &mut entry);
        }
    }
    HidInterfaceInfo {
        // 합성 경로 — Windows 장치 경로 대용(변화 감지·중복 제거·설정 창 「이 기기만 전환」 대상 찾기).
        path: format!("loc={:08X}:up={:04X}:u={:04X}:pid={:04X}:id={:X}", loc, usage_page, usage, product_id, entry),
        vendor_id,
        product_id,
        usage_page,
        usage,
        input_report_len,
        output_report_len,
        product_string,
        caps_read: usage_page != 0 || input_report_len > 0,
        device_index_hint: None,
    }
}

/// 매칭된 기기 목록(각 기기는 CFRetain — 놓을 때 CFRelease). 관리자도 함께 붙잡아 둔다.
struct DeviceList {
    manager: Cf,
    devices: Vec<IOHIDDeviceRef>,
}

impl Drop for DeviceList {
    fn drop(&mut self) {
        for d in &self.devices {
            unsafe { CFRelease(*d as CFTypeRef) };
        }
    }
}

fn copy_devices(vendor_filter: u16) -> DeviceList {
    unsafe {
        let mgr = IOHIDManagerCreate(kCFAllocatorDefault, 0);
        let mut out = DeviceList {
            manager: Cf(mgr as CFTypeRef),
            devices: Vec::new(),
        };
        if mgr.is_null() {
            return out;
        }
        if vendor_filter != 0 {
            let key = cfstr("VendorID");
            let v: i32 = vendor_filter as i32;
            let num = Cf(CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &v as *const i32 as *const c_void) as CFTypeRef);
            let keys = [key.0];
            let vals = [num.0];
            let dict = Cf(CFDictionaryCreate(
                kCFAllocatorDefault,
                keys.as_ptr(),
                vals.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            ) as CFTypeRef);
            IOHIDManagerSetDeviceMatching(mgr, dict.0 as _);
        } else {
            IOHIDManagerSetDeviceMatching(mgr, std::ptr::null());
        }
        let set = IOHIDManagerCopyDevices(mgr);
        if set.is_null() {
            return out;
        }
        let n = CFSetGetCount(set);
        let mut vals: Vec<*const c_void> = vec![std::ptr::null(); n.max(0) as usize];
        if n > 0 {
            CFSetGetValues(set, vals.as_mut_ptr());
        }
        for v in vals {
            if !v.is_null() {
                CFRetain(v);
                out.devices.push(v as IOHIDDeviceRef);
            }
        }
        CFRelease(set as CFTypeRef);
        let _ = &out.manager;
        out
    }
}

pub struct MacHidBackend;

impl HidBackend for MacHidBackend {
    fn enumerate(&self, vendor_filter: u16, usage_page_filter: u16) -> Vec<HidInterfaceInfo> {
        let list = copy_devices(vendor_filter);
        let mut out: Vec<HidInterfaceInfo> = list
            .devices
            .iter()
            .map(|d| read_info(*d))
            .filter(|i| vendor_filter == 0 || i.vendor_id == 0 || i.vendor_id == vendor_filter)
            .filter(|i| usage_page_filter == 0 || i.usage_page == usage_page_filter)
            .collect();
        // CFSet 순서는 정해져 있지 않다 — 진단·캐시가 매번 같은 순서로 나오게 경로순.
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn open(&self, info: &HidInterfaceInfo) -> Result<Box<dyn HidTransport>, String> {
        let list = copy_devices(info.vendor_id);
        let dev = list
            .devices
            .iter()
            .copied()
            .find(|d| read_info(*d).path == info.path)
            .ok_or_else(|| "장치를 찾지 못함(분리됨?)".to_string())?;
        MacTransport::open(dev, info.output_report_len.max(info.input_report_len))
    }
}

struct Inner {
    frames: Mutex<VecDeque<Vec<u8>>>,
    cond: Condvar,
}

extern "C" fn on_input(ctx: *mut c_void, _result: IOReturn, _sender: *mut c_void, _ty: u32, report_id: u32, report: *mut u8, len: CFIndex) {
    if ctx.is_null() {
        return;
    }
    let inner = unsafe { &*(ctx as *const Inner) };
    let data: &[u8] = if report.is_null() || len <= 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(report, len as usize) }
    };
    let frame = match data.first() {
        Some(&b) if b == REPORT_ID_SHORT || b == REPORT_ID_LONG => data.to_vec(),
        _ => {
            let mut f = Vec::with_capacity(data.len() + 1);
            f.push((report_id & 0xFF) as u8);
            f.extend_from_slice(data);
            f
        }
    };
    if let Ok(mut q) = inner.frames.lock() {
        q.push_back(frame);
        // 오래 안 읽힌 입력이 끝없이 쌓이지 않게.
        while q.len() > 256 {
            q.pop_front();
        }
        inner.cond.notify_one();
    }
}

/// 열린 IOHIDDevice 하나.
pub struct MacTransport {
    dev: usize,
    inner: Arc<Inner>,
    /// 콜백 문맥으로 넘긴 Arc(닫을 때 되찾는다).
    ctx: usize,
    buf: usize,
    buf_len: usize,
    out_len: usize,
}

// IOHIDDevice 쓰기·닫기는 어느 스레드에서나 된다(입력 콜백은 전용 런루프 스레드).
unsafe impl Send for MacTransport {}

impl MacTransport {
    fn open(dev: IOHIDDeviceRef, report_len: usize) -> Result<Box<dyn HidTransport>, String> {
        let r = unsafe { IOHIDDeviceOpen(dev, kIOHIDOptionsTypeNone) };
        if r != kIOReturnSuccess {
            return Err(format!("열기 실패(IOReturn 0x{:08X})", r as u32));
        }
        unsafe { CFRetain(dev as CFTypeRef) };
        let inner = Arc::new(Inner {
            frames: Mutex::new(VecDeque::new()),
            cond: Condvar::new(),
        });
        let buf_len = report_len.max(64);
        let buf = Box::into_raw(vec![0u8; buf_len].into_boxed_slice()) as *mut u8;
        let ctx = Arc::into_raw(inner.clone()) as *mut c_void;
        let rl = run_loop();
        unsafe {
            IOHIDDeviceRegisterInputReportCallback(dev, buf, buf_len as CFIndex, Some(on_input), ctx);
            if !rl.is_null() {
                IOHIDDeviceScheduleWithRunLoop(dev, rl, kCFRunLoopDefaultMode);
                CFRunLoopWakeUp(rl);
            }
        }
        Ok(Box::new(MacTransport {
            dev: dev as usize,
            inner,
            ctx: ctx as usize,
            buf: buf as usize,
            buf_len,
            out_len: report_len,
        }))
    }
}

impl HidTransport for MacTransport {
    fn output_report_len(&self) -> usize {
        self.out_len
    }

    fn write(&mut self, report: &OutReport, _timeout_ms: u32) -> bool {
        let b = report.bytes();
        if b.is_empty() {
            return false;
        }
        let r = unsafe {
            IOHIDDeviceSetReport(
                self.dev as IOHIDDeviceRef,
                kIOHIDReportTypeOutput,
                b[0] as CFIndex,
                b.as_ptr(),
                b.len() as CFIndex,
            )
        };
        r == kIOReturnSuccess
    }

    fn read(&mut self, timeout_ms: u32) -> Option<Vec<u8>> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
        let mut q = self.inner.frames.lock().ok()?;
        loop {
            if let Some(f) = q.pop_front() {
                return Some(f);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            q = self.inner.cond.wait_timeout(q, left).ok()?.0;
        }
    }
}

impl Drop for MacTransport {
    fn drop(&mut self) {
        let dev = self.dev;
        // 콜백이 도는 스레드에서 해제·빼기·닫기를 끝낸다 — 그 뒤로는 콜백이 오지 않는다.
        on_run_loop(move || unsafe {
            let d = dev as IOHIDDeviceRef;
            IOHIDDeviceRegisterInputReportCallback(d, std::ptr::null_mut(), 0, None, std::ptr::null_mut());
            let rl = CFRunLoopGetCurrent();
            IOHIDDeviceUnscheduleFromRunLoop(d, rl, kCFRunLoopDefaultMode);
            IOHIDDeviceClose(d, kIOHIDOptionsTypeNone);
        });
        unsafe {
            CFRelease(self.dev as CFTypeRef);
            drop(Arc::from_raw(self.ctx as *const Inner));
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(self.buf as *mut u8, self.buf_len)));
        }
    }
}
