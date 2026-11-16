//! 상주 앱 ↔ 설정 창 프로세스 IPC — Windows WM_COPYDATA 와 같은 종류 번호 · 같은 상태 묶음, 전달은 Unix 소켓.
//!
//! 한 번 보내기 = 연결 → [종류 u32 LE][길이 u32 LE][내용 UTF-8] → 받는 쪽이 1바이트(1) 답 → 닫기.
//! 받는 쪽은 내용을 큐에 넣고 바로 답하므로(처리는 나중) 보내는 쪽을 오래 붙잡지 않는다.
//! 상대 번호: 0 = 상주 앱(`agent.sock`), 그 밖 = 설정 창 프로세스 번호(`ui-<pid>.sock`).

use super::dialog::HWND;
use super::paths;
use super::wnd::Hwnd;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

// 설정 창 → 상주 앱
pub const HELLO: usize = 1; // 내용 = 설정 창 프로세스 번호(10진)
pub const SET_CONFIG: usize = 2; // 내용 = 평문 설정
pub const RESCAN: usize = 3;
pub const SWITCH_ONE: usize = 4; // 내용 = path \t deviceIndex \t target
pub const BYE: usize = 5;
pub const SWITCH_ALL: usize = 6; // 내용 = target
/// 두 번째 실행(명령줄) → 이미 도는 상주 앱: 설정 창 열기.
pub const SHOW: usize = 7;
// 상주 앱 → 설정 창
pub const STATE: usize = 101;
pub const SWITCH_RESULT: usize = 102; // "1" / "0"
pub const ACTIVATE: usize = 103;

pub const AGENT: HWND = HWND(0);
const MAX_LEN: usize = 4 * 1024 * 1024;

fn socket_of(to: HWND) -> PathBuf {
    if to.0 == 0 {
        paths::agent_socket()
    } else {
        paths::ui_socket(to.0 as u32)
    }
}

/// 상주 앱이 떠 있으면 그 번호(0).
pub fn find_tray() -> Option<HWND> {
    let s = UnixStream::connect(paths::agent_socket()).ok()?;
    drop(s);
    Some(AGENT)
}

/// 보내고 받는 쪽이 받았다고 답할 때까지 기다린다(최대 timeout).
pub fn send(to: HWND, kind: usize, text: &str, timeout_ms: u32) -> bool {
    let t = Some(Duration::from_millis(timeout_ms.max(50) as u64));
    let Ok(mut s) = UnixStream::connect(socket_of(to)) else {
        return false;
    };
    let _ = s.set_write_timeout(t);
    let _ = s.set_read_timeout(t);
    let bytes = text.as_bytes();
    let mut frame = Vec::with_capacity(8 + bytes.len());
    frame.extend_from_slice(&(kind as u32).to_le_bytes());
    frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(bytes);
    if s.write_all(&frame).is_err() || s.flush().is_err() {
        return false;
    }
    let mut ack = [0u8; 1];
    s.read_exact(&mut ack).is_ok() && ack[0] == 1
}

fn read_one(s: &mut UnixStream) -> Option<(usize, String)> {
    let _ = s.set_read_timeout(Some(Duration::from_millis(3000)));
    let mut head = [0u8; 8];
    s.read_exact(&mut head).ok()?;
    let kind = u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize;
    let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
    if len > MAX_LEN {
        return None;
    }
    let mut body = vec![0u8; len];
    s.read_exact(&mut body).ok()?;
    Some((kind, String::from_utf8_lossy(&body).to_string()))
}

/// 받는 쪽 — 소켓을 열고 받을 때마다 on_message(종류, 내용)를 이 스레드에서 부른다(가벼운 일만: 큐에 넣기 등).
/// 같은 자리에 남은 옛 소켓 파일은 지우고 연다. 실패하면 false.
pub fn listen(path: PathBuf, on_message: impl Fn(usize, String) + Send + 'static) -> bool {
    paths::ensure_support_dir();
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        return false;
    };
    // 같은 사용자만(소켓 파일 권한 0600).
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    std::thread::Builder::new()
        .name("LogiFlow.Ipc".into())
        .spawn(move || {
            for conn in listener.incoming() {
                let Ok(mut s) = conn else {
                    continue;
                };
                if let Some((kind, text)) = read_one(&mut s) {
                    on_message(kind, text);
                    let _ = s.write_all(&[1]);
                }
            }
        })
        .is_ok()
}

pub fn hwnd_text(h: HWND) -> String {
    Hwnd::of(h).0.to_string()
}

pub fn parse_hwnd(s: &str) -> Option<Hwnd> {
    s.trim().parse::<isize>().ok().filter(|v| *v > 0).map(Hwnd)
}

/// 상주 앱 상태 묶음(Windows 와 같은 글 모양).
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
