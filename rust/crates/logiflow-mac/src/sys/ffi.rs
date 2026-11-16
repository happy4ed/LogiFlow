//! C API 선언 — IOKit HID · CoreGraphics 화면/커서 · libdispatch · CFRunLoop 블록 · CF 값 꺼내기.
//! (AppKit 클래스는 objc2-app-kit 으로, C 함수는 여기서 직접 선언한다 — 서명이 단순하고 바뀌지 않는다.)

// 꺼내기 도우미는 CF API 가 돌려준 값(또는 null)만 받는다 — 널 검사 뒤 형식(TypeID)을 확인하고 읽는다.
#![allow(non_upper_case_globals, non_camel_case_types, clippy::upper_case_acronyms, clippy::not_unsafe_ptr_arg_deref)]

use core_foundation_sys::base::{kCFAllocatorDefault, CFAllocatorRef, CFGetTypeID, CFIndex, CFRelease, CFTypeRef};
use core_foundation_sys::data::{CFDataGetBytePtr, CFDataGetLength, CFDataGetTypeID};
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::number::{kCFNumberSInt64Type, CFBooleanGetTypeID, CFBooleanGetValue, CFBooleanRef, CFNumberGetTypeID, CFNumberGetValue, CFNumberRef};
use core_foundation_sys::runloop::CFRunLoopRef;
use core_foundation_sys::set::CFSetRef;
use core_foundation_sys::string::{
    kCFStringEncodingUTF8, CFStringCreateWithBytes, CFStringGetCString, CFStringGetLength, CFStringGetMaximumSizeForEncoding, CFStringGetTypeID, CFStringRef,
};
use std::ffi::c_void;

pub type IOHIDManagerRef = *mut c_void;
pub type IOHIDDeviceRef = *mut c_void;
pub type IOReturn = i32;
pub type io_service_t = u32;
pub const kIOReturnSuccess: IOReturn = 0;
pub const kIOHIDReportTypeOutput: u32 = 1;
pub const kIOHIDOptionsTypeNone: u32 = 0;

pub type IOHIDReportCallback =
    extern "C" fn(context: *mut c_void, result: IOReturn, sender: *mut c_void, report_type: u32, report_id: u32, report: *mut u8, report_length: CFIndex);

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    pub fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: u32) -> IOHIDManagerRef;
    pub fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);
    pub fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;
    pub fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;
    pub fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: u32) -> IOReturn;
    pub fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: u32) -> IOReturn;
    pub fn IOHIDDeviceScheduleWithRunLoop(device: IOHIDDeviceRef, run_loop: CFRunLoopRef, mode: CFStringRef);
    pub fn IOHIDDeviceUnscheduleFromRunLoop(device: IOHIDDeviceRef, run_loop: CFRunLoopRef, mode: CFStringRef);
    pub fn IOHIDDeviceRegisterInputReportCallback(
        device: IOHIDDeviceRef,
        report: *mut u8,
        report_length: CFIndex,
        callback: Option<IOHIDReportCallback>,
        context: *mut c_void,
    );
    pub fn IOHIDDeviceSetReport(device: IOHIDDeviceRef, report_type: u32, report_id: CFIndex, report: *const u8, report_length: CFIndex) -> IOReturn;
    pub fn IOHIDDeviceGetService(device: IOHIDDeviceRef) -> io_service_t;
    pub fn IORegistryEntryGetRegistryEntryID(entry: io_service_t, entry_id: *mut u64) -> i32;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGGetActiveDisplayList(max_displays: u32, active_displays: *mut u32, display_count: *mut u32) -> i32;
    pub fn CGDisplayBounds(display: u32) -> CGRect;
    pub fn CGMainDisplayID() -> u32;
    pub fn CGEventCreate(source: *const c_void) -> *mut c_void;
    pub fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
}

// libdispatch(libSystem) — 메인 큐에 일 넣기.
unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}

pub fn dispatch_main_async(context: *mut c_void, work: extern "C" fn(*mut c_void)) {
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, context, work) }
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub fn CFRunLoopPerformBlock(rl: CFRunLoopRef, mode: CFTypeRef, block: &block2::Block<dyn Fn()>);
}

/// CF 객체 소유(놓을 때 CFRelease).
pub struct Cf(pub CFTypeRef);

impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}

pub fn cfstr(s: &str) -> Cf {
    Cf(unsafe { CFStringCreateWithBytes(kCFAllocatorDefault, s.as_ptr(), s.len() as CFIndex, kCFStringEncodingUTF8, 0) } as CFTypeRef)
}

pub fn cf_string(r: CFTypeRef) -> Option<String> {
    if r.is_null() || unsafe { CFGetTypeID(r) } != unsafe { CFStringGetTypeID() } {
        return None;
    }
    let s = r as CFStringRef;
    unsafe {
        let len = CFStringGetLength(s);
        let max = CFStringGetMaximumSizeForEncoding(len, kCFStringEncodingUTF8) + 1;
        let mut buf = vec![0u8; max.max(1) as usize];
        if CFStringGetCString(s, buf.as_mut_ptr() as *mut _, max, kCFStringEncodingUTF8) == 0 {
            return None;
        }
        let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
        buf.truncate(end);
        String::from_utf8(buf).ok()
    }
}

pub fn cf_i64(r: CFTypeRef) -> Option<i64> {
    if r.is_null() || unsafe { CFGetTypeID(r) } != unsafe { CFNumberGetTypeID() } {
        return None;
    }
    let mut v: i64 = 0;
    let ok = unsafe { CFNumberGetValue(r as CFNumberRef, kCFNumberSInt64Type, &mut v as *mut i64 as *mut c_void) };
    if ok {
        Some(v)
    } else {
        None
    }
}

pub fn cf_bool(r: CFTypeRef) -> Option<bool> {
    if r.is_null() {
        return None;
    }
    if unsafe { CFGetTypeID(r) } == unsafe { CFBooleanGetTypeID() } {
        return Some(unsafe { CFBooleanGetValue(r as CFBooleanRef) });
    }
    cf_i64(r).map(|v| v != 0)
}

pub fn cf_bytes(r: CFTypeRef) -> Option<Vec<u8>> {
    if r.is_null() || unsafe { CFGetTypeID(r) } != unsafe { CFDataGetTypeID() } {
        return None;
    }
    unsafe {
        let len = CFDataGetLength(r as _);
        let p = CFDataGetBytePtr(r as _);
        if p.is_null() || len <= 0 {
            return Some(Vec::new());
        }
        Some(std::slice::from_raw_parts(p, len as usize).to_vec())
    }
}
