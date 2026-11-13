//! 창 도우미 — 클로저 핸들러 창, 메시지 루프, 다른 스레드에서 UI 스레드로 일 넘기기(post).
//!
//! 핸들러는 상태를 빌리지 않는 `Fn` 이라 재진입(모달 메뉴·SendMessage)에도 안전하다.
//! 상태는 각 모듈의 thread_local 에 두고 짧게만 빌린다.

use crate::util::wide;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

pub type Handler = Rc<dyn Fn(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT>>;

/// HWND 를 스레드 사이로 옮기기 위한 값(포인터를 정수로).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Hwnd(pub isize);
unsafe impl Send for Hwnd {}
unsafe impl Sync for Hwnd {}
impl Hwnd {
    pub fn of(h: HWND) -> Self {
        Hwnd(h.0 as isize)
    }
    pub fn get(self) -> HWND {
        HWND(self.0 as *mut _)
    }
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

pub const HWND_MESSAGE_PARENT: HWND = HWND(-3isize as *mut _);
pub const WM_APP_RUN: u32 = WM_APP + 1;

thread_local! {
    static REGISTERED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let cs = &*(lp.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Handler;
        if !ptr.is_null() {
            let h: Handler = (*ptr).clone();
            if let Some(r) = h(hwnd, msg, wp, lp) {
                return r;
            }
        }
        if msg == WM_NCDESTROY && !ptr.is_null() {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr as *mut Handler));
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
}

pub fn ensure_class(class: &str, style: WNDCLASS_STYLES) {
    let fresh = REGISTERED.with(|r| r.borrow_mut().insert(class.to_string()));
    if !fresh {
        return;
    }
    let name = wide(class);
    // 클래스 이름 버퍼는 프로세스 수명 동안 유지.
    let name: &'static [u16] = Box::leak(name.into_boxed_slice());
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(name.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wc);
    }
}

/// 핸들러 창 만들기. parent = HWND_MESSAGE_PARENT 면 메시지 전용 창.
pub fn create(
    class: &str,
    title: &str,
    ex_style: WINDOW_EX_STYLE,
    style: WINDOW_STYLE,
    rect: (i32, i32, i32, i32),
    parent: Option<HWND>,
    handler: Handler,
) -> Option<HWND> {
    ensure_class(class, WNDCLASS_STYLES(0));
    let cls = wide(class);
    let ttl = wide(title);
    let boxed: *mut Handler = Box::into_raw(Box::new(handler));
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        match CreateWindowExW(
            ex_style,
            PCWSTR(cls.as_ptr()),
            PCWSTR(ttl.as_ptr()),
            style,
            rect.0,
            rect.1,
            rect.2,
            rect.3,
            parent,
            None,
            Some(hinst.into()),
            Some(boxed as *const _),
        ) {
            Ok(h) => Some(h),
            Err(_) => {
                drop(Box::from_raw(boxed));
                None
            }
        }
    }
}

/// 메시지 전용 창.
pub fn message_window(class: &str, handler: Handler) -> Option<HWND> {
    create(
        class,
        class,
        WINDOW_EX_STYLE(0),
        WINDOW_STYLE(0),
        (0, 0, 0, 0),
        Some(HWND_MESSAGE_PARENT),
        handler,
    )
}

/// 표준 메시지 루프(WM_QUIT 까지).
pub fn run_message_loop() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn quit() {
    unsafe { PostQuitMessage(0) }
}

// ---- UI 스레드로 일 넘기기 ----

type Job = Box<dyn FnOnce() + Send>;

struct Dispatcher {
    hwnd: Hwnd,
    tx: Mutex<Sender<Job>>,
}

static DISPATCHER: OnceLock<Dispatcher> = OnceLock::new();

thread_local! {
    static JOBS: RefCell<Option<Receiver<Job>>> = const { RefCell::new(None) };
}

/// 현재 스레드(메시지 루프를 도는 UI 스레드)를 post 대상으로 정한다. 프로세스당 한 번.
pub fn init_dispatcher() {
    let (tx, rx) = channel::<Job>();
    JOBS.with(|j| *j.borrow_mut() = Some(rx));
    let handler: Handler = Rc::new(|_h, msg, _w, _l| {
        if msg == WM_APP_RUN {
            loop {
                let job = JOBS.with(|j| j.borrow().as_ref().and_then(|rx| rx.try_recv().ok()));
                match job {
                    Some(f) => f(),
                    None => break,
                }
            }
            return Some(LRESULT(0));
        }
        None
    });
    if let Some(h) = message_window("LogiFlowRs.Dispatcher", handler) {
        let _ = DISPATCHER.set(Dispatcher {
            hwnd: Hwnd::of(h),
            tx: Mutex::new(tx),
        });
    }
}

/// 아무 스레드에서나 — UI 스레드에서 f 를 실행.
pub fn post(f: impl FnOnce() + Send + 'static) {
    if let Some(d) = DISPATCHER.get() {
        if d.tx.lock().map(|tx| tx.send(Box::new(f))).is_ok() {
            unsafe {
                let _ = PostMessageW(Some(d.hwnd.get()), WM_APP_RUN, WPARAM(0), LPARAM(0));
            }
        }
    }
}
