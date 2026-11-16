//! 공용 도우미 — 실행 파일 자리 · 자기 자신 띄우기 · 메시지 상자(NSAlert) · 단추 고르는 확인 상자 · 파일 열기 · 단조 시계.

use super::dialog::{localize, HWND, IDNO, IDOK, IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_YESNO, MESSAGEBOX_RESULT, MESSAGEBOX_STYLE};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSAlertStyle, NSApplication};
use objc2_foundation::NSString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub fn exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_default()
}

/// 설정 창이 기록·설정 자리로 쓰는 폴더. Mac 은 번들 안이 아니라 Application Support.
pub fn exe_dir() -> PathBuf {
    super::paths::support_dir()
}

/// 이 실행 파일이 들어 있는 .app 번들(없으면 None — 번들 밖에서 실행).
pub fn bundle_path() -> Option<PathBuf> {
    let exe = exe_path();
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    if macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && app.extension().map(|e| e == "app").unwrap_or(false) {
        Some(app.to_path_buf())
    } else {
        None
    }
}

pub fn spawn_self(args: &[&str]) -> Option<std::process::Child> {
    Command::new(exe_path()).args(args).spawn().ok()
}

fn ns(s: &str) -> objc2::rc::Retained<NSString> {
    NSString::from_str(&localize(s).replace("\r\n", "\n"))
}

/// 우리 앱을 앞으로(상주 앱이 띄운 상자가 다른 앱 뒤에 숨지 않게).
fn bring_front(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}

/// Windows MessageBox 대응. 제목은 굵은 첫 줄, 본문은 그 아래. 메인 스레드가 아니면 띄우지 않는다.
pub fn message_box(_owner: Option<HWND>, text: &str, title: &str, style: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    let Some(mtm) = MainThreadMarker::new() else {
        return if style.has(MB_YESNO) { IDNO } else { IDOK };
    };
    bring_front(mtm);
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(title));
    alert.setInformativeText(&ns(text));
    alert.setAlertStyle(if style.has(MB_ICONINFORMATION) || style.has(MB_ICONQUESTION) {
        NSAlertStyle::Informational
    } else {
        NSAlertStyle::Warning
    });
    if style.has(MB_YESNO) {
        alert.addButtonWithTitle(&ns("예"));
        alert.addButtonWithTitle(&ns("아니요"));
        if alert.runModal() == NSAlertFirstButtonReturn {
            IDYES
        } else {
            IDNO
        }
    } else {
        alert.addButtonWithTitle(&ns("확인"));
        alert.runModal();
        IDOK
    }
}

/// Windows TaskDialog 대응 — 단추 이름을 정할 수 있는 확인 상자. buttons = (번호, 이름) + 「취소」.
/// 고른 번호, 취소면 0.
pub fn task_dialog(_owner: Option<HWND>, _title: &str, main: &str, content: &str, buttons: &[(i32, &str)]) -> i32 {
    let Some(mtm) = MainThreadMarker::new() else {
        return 0;
    };
    bring_front(mtm);
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(main));
    alert.setInformativeText(&ns(content));
    alert.setAlertStyle(NSAlertStyle::Warning);
    for (_, label) in buttons {
        alert.addButtonWithTitle(&ns(label));
    }
    alert.addButtonWithTitle(&ns("취소"));
    let r = alert.runModal() - NSAlertFirstButtonReturn;
    if r >= 0 && (r as usize) < buttons.len() {
        buttons[r as usize].0
    } else {
        0
    }
}

/// 폴더·파일 열기 — Windows 의 탐색기·메모장 대신 Finder · 텍스트 편집기.
pub fn open_with(program: &str, target: &Path) {
    let mut c = Command::new("/usr/bin/open");
    if program.to_ascii_lowercase().contains("notepad") {
        c.arg("-t");
    }
    let _ = c.arg(target).spawn();
}

/// 프로세스 시작 이후 밀리초(단조 시계).
pub fn tick_ms() -> u64 {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}
