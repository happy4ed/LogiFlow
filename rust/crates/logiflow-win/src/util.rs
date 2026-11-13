//! 공용 도우미 — 넓은 문자열, 실행 파일 경로, 숨은 프로세스 실행, UAC 상승 실행, 메시지 상자.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MESSAGEBOX_RESULT, MESSAGEBOX_STYLE, SW_SHOWNORMAL};

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub fn exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("LogiFlowRs.exe"))
}

pub fn exe_dir() -> PathBuf {
    exe_path().parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
}

/// 창 없이 프로세스를 돌리고 종료 코드(시간 초과·실패 = 1). 출력은 (stdout+stderr) 로 돌려준다.
pub fn run_hidden(program: &str, args: &str, timeout_ms: u64) -> (i32, String) {
    let mut cmd = Command::new(program);
    cmd.raw_arg(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (1, e.to_string()),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let out = child.wait_with_output().ok();
                let text = out
                    .map(|o| format!("{} {}", String::from_utf8_lossy(&o.stderr), String::from_utf8_lossy(&o.stdout)))
                    .unwrap_or_default();
                return (st.code().unwrap_or(1), text);
            }
            Ok(None) => {
                if start.elapsed() > Duration::from_millis(timeout_ms) {
                    let _ = child.kill();
                    return (1, "timeout".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return (1, e.to_string()),
        }
    }
}

/// 자기 자신을 UAC 상승(runas)으로 실행하고 끝날 때까지(최대 timeout) 기다려 종료 코드 0 이면 true.
/// 사용자가 UAC 를 취소하면 false.
pub fn run_elevated_self(args: &str, timeout_ms: u32) -> bool {
    let exe = HSTRING::from(exe_path().as_os_str());
    let params = HSTRING::from(args);
    let verb = HSTRING::from("runas");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(exe.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe {
        if ShellExecuteExW(&mut info).is_err() || info.hProcess.is_invalid() {
            return false;
        }
        let w = WaitForSingleObject(info.hProcess, timeout_ms);
        let mut code = 1u32;
        let ok = w == WAIT_OBJECT_0 && GetExitCodeProcess(info.hProcess, &mut code).is_ok();
        let _ = CloseHandle(info.hProcess);
        ok && code == 0
    }
}

/// 자기 자신을 일반 권한으로 띄운다(설정 창 등). 실패 시 None.
pub fn spawn_self(args: &[&str]) -> Option<std::process::Child> {
    Command::new(exe_path()).args(args).spawn().ok()
}

pub fn message_box(owner: Option<HWND>, text: &str, title: &str, style: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    let t = HSTRING::from(text);
    let c = HSTRING::from(title);
    unsafe { MessageBoxW(owner, PCWSTR(t.as_ptr()), PCWSTR(c.as_ptr()), style) }
}

/// 탐색기·메모장으로 열기(C# Process.Start("explorer.exe", "\"dir\"") 와 같게).
pub fn open_with(program: &str, target: &std::path::Path) {
    let _ = Command::new(program).raw_arg(format!("\"{}\"", target.display())).spawn();
}

/// 프로세스 시작 이후 밀리초(C# Environment.TickCount 대용 — 단조 시계).
pub fn tick_ms() -> u64 {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}
