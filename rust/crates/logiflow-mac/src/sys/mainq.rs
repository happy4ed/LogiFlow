//! 메인 스레드에 일 넣기(libdispatch 메인 큐). Windows `wnd::post` 대응 — 메뉴 막대·가장자리 감시·glow 상태는
//! 메인 스레드 전용이고, 스캔·전환·클립보드 수신은 백그라운드 스레드에서 끝난 뒤 여기로 결과를 넘긴다.
//! 메인 큐는 메뉴가 열려 있는 동안(이벤트 추적 모드)에도 돈다.

use super::ffi::dispatch_main_async;
use std::ffi::c_void;

type Job = Box<dyn FnOnce() + Send + 'static>;

extern "C" fn trampoline(ctx: *mut c_void) {
    let job: Box<Job> = unsafe { Box::from_raw(ctx as *mut Job) };
    job();
}

pub fn post(f: impl FnOnce() + Send + 'static) {
    let job: Box<Job> = Box::new(Box::new(f));
    dispatch_main_async(Box::into_raw(job) as *mut c_void, trampoline);
}

/// 지금 메인 스레드인지.
pub fn is_main() -> bool {
    objc2::MainThreadMarker::new().is_some()
}
