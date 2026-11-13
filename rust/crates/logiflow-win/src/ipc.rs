//! 트레이 ↔ 설정 창 프로세스 IPC — WM_COPYDATA(같은 사용자 세션, 메시지 전용 창).
//! 트레이가 상태(설정·기기 목록·HID)의 단일 주인. 설정 창은 요청만 보내고 STATE 로 갱신받는다.

use crate::paths::TRAY_IPC_CLASS;
use crate::util::wide;
use crate::wnd::Hwnd;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, SendMessageTimeoutW, SMTO_ABORTIFHUNG, WM_COPYDATA};

// 설정 창 → 트레이
pub const HELLO: usize = 1; // 내용 = 설정 창 IPC 창 번호(10진)
pub const SET_CONFIG: usize = 2; // 내용 = 평문 설정
pub const RESCAN: usize = 3;
pub const SWITCH_ONE: usize = 4; // 내용 = path \t deviceIndex \t target
pub const BYE: usize = 5;
// 트레이 → 설정 창
pub const STATE: usize = 101;
pub const SWITCH_RESULT: usize = 102; // "1" / "0"
pub const ACTIVATE: usize = 103;

pub const UI_IPC_CLASS: &str = "LogiFlowRs.UiIpc";

pub fn find_tray() -> Option<HWND> {
    let cls = wide(TRAY_IPC_CLASS);
    unsafe {
        FindWindowExW(Some(crate::wnd::HWND_MESSAGE_PARENT), None, PCWSTR(cls.as_ptr()), PCWSTR::null())
            .ok()
            .filter(|h| !h.is_invalid())
    }
}

/// 보내고 받는 쪽 처리까지 기다린다(최대 timeout).
pub fn send(to: HWND, kind: usize, text: &str, timeout_ms: u32) -> bool {
    let bytes = text.as_bytes();
    let cds = COPYDATASTRUCT {
        dwData: kind,
        cbData: bytes.len() as u32,
        lpData: bytes.as_ptr() as *mut _,
    };
    let mut res = 0usize;
    unsafe {
        SendMessageTimeoutW(
            to,
            WM_COPYDATA,
            WPARAM(0),
            LPARAM(&cds as *const _ as isize),
            SMTO_ABORTIFHUNG,
            timeout_ms,
            Some(&mut res),
        )
        .0 != 0
    }
}

/// WM_COPYDATA lparam 해석.
pub fn read(lp: LPARAM) -> (usize, String) {
    unsafe {
        let cds = &*(lp.0 as *const COPYDATASTRUCT);
        let s = if cds.cbData == 0 || cds.lpData.is_null() {
            String::new()
        } else {
            String::from_utf8_lossy(std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize)).to_string()
        };
        (cds.dwData, s)
    }
}

pub fn hwnd_text(h: HWND) -> String {
    Hwnd::of(h).0.to_string()
}

pub fn parse_hwnd(s: &str) -> Option<Hwnd> {
    s.trim().parse::<isize>().ok().map(Hwnd)
}

/// 트레이 상태 묶음.
pub struct State {
    pub scanning: bool,
    pub clip_running: bool,
    pub config_plain: String,
    pub devices: String,
}

pub fn encode_state(s: &State) -> String {
    format!(
        "scanning={}\nclipRunning={}\n[config]\n{}\n[devices]\n{}",
        if s.scanning { 1 } else { 0 },
        if s.clip_running { 1 } else { 0 },
        s.config_plain,
        s.devices
    )
}

pub fn decode_state(t: &str) -> State {
    let mut st = State {
        scanning: false,
        clip_running: false,
        config_plain: String::new(),
        devices: String::new(),
    };
    let mut section = 0;
    for line in t.split('\n') {
        let l = line.trim_end_matches('\r');
        match l {
            "[config]" => section = 1,
            "[devices]" => section = 2,
            _ => match section {
                0 => {
                    if let Some(v) = l.strip_prefix("scanning=") {
                        st.scanning = v == "1";
                    } else if let Some(v) = l.strip_prefix("clipRunning=") {
                        st.clip_running = v == "1";
                    }
                }
                1 => {
                    st.config_plain.push_str(l);
                    st.config_plain.push('\n');
                }
                _ => {
                    st.devices.push_str(l);
                    st.devices.push('\n');
                }
            },
        }
    }
    st
}
