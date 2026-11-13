//! 세션·데스크톱·워커 투입 — C# `DesktopProbe` 의 세션0 런처·SpawnWorker·입력 데스크톱 도우미 이식.
//! 세션0 서비스(SYSTEM)는 활성 콘솔 세션의 winlogon.exe 토큰을 복제해 지정 데스크톱에 워커를 띄운다.

use crate::paths;
use crate::util::{exe_path, from_wide, wide};
use logiflow_core::log::{append_utf8, newline};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Security::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::RemoteDesktop::{ProcessIdToSessionId, WTSGetActiveConsoleSessionId};
use windows::Win32::System::StationsAndDesktops::*;
use windows::Win32::System::Threading::*;

pub const NO_SESSION: u32 = 0xFFFF_FFFF;

/// `%ProgramData%\LogiFlowRs\desktop-probe.log` 에 한 줄(시각 HH:mm:ss.fff).
pub fn probe_log(line: &str) {
    let path = paths::probe_log_path();
    let dir = paths::machine_dir();
    let stamped = format!("{}  {}{}", local_time_hms(), line, newline());
    append_utf8(&dir, &path, &stamped);
}

/// 지역 시각 HH:mm:ss.fff (C# DateTime.Now.ToString("HH:mm:ss.fff")).
pub fn local_time_hms() -> String {
    unsafe {
        let st = windows::Win32::System::SystemInformation::GetLocalTime();
        format!("{:02}:{:02}:{:02}.{:03}", st.wHour, st.wMinute, st.wSecond, st.wMilliseconds)
    }
}

pub fn service_log(s: &str) {
    probe_log(&format!("[서비스] {}", s));
}

pub fn active_console_session() -> u32 {
    unsafe { WTSGetActiveConsoleSessionId() }
}

pub fn current_session() -> u32 {
    let mut sid = NO_SESSION;
    unsafe {
        let _ = ProcessIdToSessionId(GetCurrentProcessId(), &mut sid);
    }
    sid
}

/// 실행 계정 이름(로그용, 예: NT AUTHORITY\SYSTEM).
pub fn who() -> String {
    std::env::var("USERDOMAIN")
        .ok()
        .zip(std::env::var("USERNAME").ok())
        .map(|(d, u)| format!("{}\\{}", d, u))
        .unwrap_or_else(|| "?".into())
}

fn find_process_in_session(exe: &str, session: u32) -> u32 {
    unsafe {
        let snap = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        let mut pe = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found = 0;
        if Process32FirstW(snap, &mut pe).is_ok() {
            loop {
                if from_wide(&pe.szExeFile).eq_ignore_ascii_case(exe) {
                    let mut sid = 0u32;
                    if ProcessIdToSessionId(pe.th32ProcessID, &mut sid).is_ok() && sid == session {
                        found = pe.th32ProcessID;
                        break;
                    }
                }
                if Process32NextW(snap, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        found
    }
}

/// 서비스가 소유하는 워커 프로세스 핸들(HANDLE 을 스레드 사이로).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proc(pub isize);
unsafe impl Send for Proc {}

impl Proc {
    pub const NONE: Proc = Proc(0);
    pub fn handle(self) -> HANDLE {
        HANDLE(self.0 as *mut _)
    }
    pub fn alive(self) -> bool {
        self.0 != 0 && unsafe { WaitForSingleObject(self.handle(), 0) } == WAIT_TIMEOUT
    }
    /// 강제 종료(PID 패턴 매칭 없이 핸들로만) + 핸들 닫기.
    pub fn kill(self) {
        if self.0 == 0 {
            return;
        }
        unsafe {
            let _ = TerminateProcess(self.handle(), 0);
            let _ = CloseHandle(self.handle());
        }
    }
}

/// 활성 콘솔 세션의 desktop(예: `winsta0\winlogon`)에 winlogon(SYSTEM) 토큰으로 자기 자신을 child_arg 로 띄운다.
/// 성공 시 (프로세스 핸들, pid, 세션). 실패 이유는 서비스 로그에.
pub fn spawn_worker(child_arg: &str, desktop: &str, log: &dyn Fn(&str)) -> Option<(Proc, u32, u32)> {
    unsafe {
        let session = WTSGetActiveConsoleSessionId();
        if session == NO_SESSION {
            log("활성 콘솔 세션 없음 — 대기");
            return None;
        }
        let pid = find_process_in_session("winlogon.exe", session);
        if pid == 0 {
            log(&format!("winlogon(세션 {}) 못 찾음 — 대기", session));
            return None;
        }
        let hproc = match OpenProcess(PROCESS_QUERY_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(e) => {
                log(&format!("OpenProcess(winlogon) 실패 err={}", e.code().0 & 0xFFFF));
                return None;
            }
        };
        let mut tok = HANDLE::default();
        if let Err(e) = OpenProcessToken(hproc, TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY, &mut tok) {
            log(&format!("OpenProcessToken 실패 err={}", e.code().0 & 0xFFFF));
            let _ = CloseHandle(hproc);
            return None;
        }
        let mut dup = HANDLE::default();
        if let Err(e) = DuplicateTokenEx(
            tok,
            TOKEN_ACCESS_MASK(0x0200_0000), /* MAXIMUM_ALLOWED */
            None,
            SecurityImpersonation,
            TokenPrimary,
            &mut dup,
        ) {
            log(&format!("DuplicateTokenEx 실패 err={}", e.code().0 & 0xFFFF));
            let _ = CloseHandle(tok);
            let _ = CloseHandle(hproc);
            return None;
        }
        let mut desk = wide(desktop);
        let si = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            lpDesktop: PWSTR(desk.as_mut_ptr()),
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();
        let mut cmd = wide(&format!("\"{}\" {}", exe_path().display(), child_arg));
        let ok = CreateProcessAsUserW(
            Some(dup),
            PCWSTR::null(),
            Some(PWSTR(cmd.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &si,
            &mut pi,
        );
        let result = match ok {
            Err(e) => {
                log(&format!("CreateProcessAsUser({}) 실패 err={}", desktop, e.code().0 & 0xFFFF));
                None
            }
            Ok(()) => {
                let _ = CloseHandle(pi.hThread);
                log(&format!("워커 투입 {} ({}) pid={} 세션={}", child_arg, desktop, pi.dwProcessId, session));
                Some((Proc(pi.hProcess.0 as isize), pi.dwProcessId, session))
            }
        };
        let _ = CloseHandle(dup);
        let _ = CloseHandle(tok);
        let _ = CloseHandle(hproc);
        result
    }
}

// ---- 입력 데스크톱 ----

pub const DESKTOP_READOBJECTS: u32 = 0x0001;
pub const DESKTOP_WRITEOBJECTS: u32 = 0x0080;

pub fn desktop_name(h: HDESK) -> String {
    let mut buf = [0u16; 128];
    let mut needed = 0u32;
    unsafe {
        if GetUserObjectInformationW(HANDLE(h.0), UOI_NAME, Some(buf.as_mut_ptr() as *mut _), 256, Some(&mut needed)).is_ok() {
            return from_wide(&buf);
        }
    }
    "(이름?)".into()
}

pub fn open_input_desktop() -> Option<HDESK> {
    unsafe {
        OpenInputDesktop(
            DESKTOP_CONTROL_FLAGS(0),
            false,
            DESKTOP_ACCESS_FLAGS(DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS),
        )
        .ok()
    }
}

pub fn thread_desktop() -> HDESK {
    unsafe { GetThreadDesktop(GetCurrentThreadId()).unwrap_or_default() }
}

pub fn set_thread_desktop(h: HDESK) -> Result<(), u32> {
    unsafe { SetThreadDesktop(h).map_err(|e| (e.code().0 as u32) & 0xFFFF) }
}

pub fn close_desktop(h: HDESK) {
    unsafe {
        let _ = CloseDesktop(h);
    }
}

pub fn last_error() -> u32 {
    unsafe { GetLastError().0 }
}
