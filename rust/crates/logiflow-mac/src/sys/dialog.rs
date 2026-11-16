//! 설정 창이 쓰는 대화 상자 모양 — Windows `logiflow-win::dialog` 와 같은 이름(메시지 상자 상수 · 좌표 변환 ·
//! 이 OS 에 있는 묶음). Mac 은 NSAlert · NSMenu 로 그린다(`util` · `notify` · `prompt`).

#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

use objc2_app_kit::NSView;
use objc2_foundation::NSPoint;
use std::borrow::Cow;

/// 창(설정 창 NSView 주소) 또는 IPC 상대 번호. 0 = 없음.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HWND(pub isize);

impl HWND {
    pub fn is_invalid(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MESSAGEBOX_STYLE(pub u32);

impl std::ops::BitOr for MESSAGEBOX_STYLE {
    type Output = Self;
    fn bitor(self, o: Self) -> Self {
        MESSAGEBOX_STYLE(self.0 | o.0)
    }
}

impl MESSAGEBOX_STYLE {
    pub fn has(self, f: MESSAGEBOX_STYLE) -> bool {
        f.0 != 0 && (self.0 & f.0) == f.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MESSAGEBOX_RESULT(pub i32);

pub const MB_OK: MESSAGEBOX_STYLE = MESSAGEBOX_STYLE(0x0);
pub const MB_YESNO: MESSAGEBOX_STYLE = MESSAGEBOX_STYLE(0x4);
pub const MB_ICONQUESTION: MESSAGEBOX_STYLE = MESSAGEBOX_STYLE(0x20);
pub const MB_ICONWARNING: MESSAGEBOX_STYLE = MESSAGEBOX_STYLE(0x30);
pub const MB_ICONINFORMATION: MESSAGEBOX_STYLE = MESSAGEBOX_STYLE(0x40);
pub const IDOK: MESSAGEBOX_RESULT = MESSAGEBOX_RESULT(1);
pub const IDYES: MESSAGEBOX_RESULT = MESSAGEBOX_RESULT(6);
pub const IDNO: MESSAGEBOX_RESULT = MESSAGEBOX_RESULT(7);

/// 설정 창 좌표(point, 창 안쪽 왼쪽 위 기준) → 화면 좌표(Cocoa, 주 화면 왼쪽 아래 기준 point). 메뉴 위치용.
pub fn to_screen(h: Option<HWND>, x: f32, y: f32, _pixels_per_point: f32) -> (i32, i32) {
    let Some(h) = h.filter(|h| !h.is_invalid()) else {
        return (x as i32, y as i32);
    };
    if !super::mainq::is_main() {
        return (x as i32, y as i32);
    }
    // h = winit 이 만든 NSView(뒤집힌 좌표: 왼쪽 위 원점). 창 → 화면으로 두 번 바꾼다.
    let view: &NSView = unsafe { &*(h.0 as *const NSView) };
    let Some(win) = view.window() else {
        return (x as i32, y as i32);
    };
    let in_win = view.convertPoint_toView(NSPoint::new(x as f64, y as f64), None);
    let p = win.convertPointToScreen(in_win);
    (p.x.round() as i32, p.y.round() as i32)
}

/// 이 OS 에 있는 묶음. Mac 은 로그인 전에도 실행(Windows 서비스) · 로그인 화면 테스트 · Windows 방화벽 ·
/// 기존 LogiFlow(C# 판) 정리가 없다. 클립보드 수신은 macOS 가 처음 한 번 허용을 묻는다.
pub struct Caps {
    pub prelogon: bool,
    pub probe: bool,
    pub firewall: bool,
    pub legacy: bool,
}

pub const CAPS: Caps = Caps {
    prelogon: false,
    probe: false,
    firewall: false,
    legacy: false,
};

/// 화면 글 중 Windows 에만 맞는 말을 Mac 말로(공용 설정 창 글은 Windows 기준으로 쓰여 있다).
const WORDS: &[(&str, &str)] = &[
    ("Windows 시작 시 실행", "로그인 시 실행"),
    (
        "Windows 에 로그인하면 LogiFlow 가 알림 영역에서 자동으로 시작합니다.",
        "Mac 에 로그인하면 LogiFlow 가 메뉴 막대에서 자동으로 시작합니다.",
    ),
    (
        "LogiFlow 가 설치된 폴더 안 logs 폴더에 있습니다.",
        "~/Library/Application Support/LogiFlowRs 안 logs 폴더에 있습니다.",
    ),
    ("트레이 메뉴", "메뉴 막대 메뉴"),
    ("알림 영역", "메뉴 막대"),
    // 조사는 끝소리에 맞춘다(「PC 가」 → 「Mac 이」).
    ("이 PC 가", "이 Mac 이"),
    ("이 PC 는", "이 Mac 은"),
    ("이 PC 로", "이 Mac 으로"),
    ("이 PC 를", "이 Mac 을"),
    ("이 PC 와", "이 Mac 과"),
    ("이 PC", "이 Mac"),
];

pub fn localize(s: &str) -> Cow<'_, str> {
    if !WORDS.iter().any(|(from, _)| s.contains(from)) {
        return Cow::Borrowed(s);
    }
    let mut out = s.to_string();
    for (from, to) in WORDS {
        if out.contains(from) {
            out = out.replace(from, to);
        }
    }
    Cow::Owned(out)
}
