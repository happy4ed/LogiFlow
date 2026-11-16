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
/// Mac 판 첫 사용 점검(11-16): Mac 에서 「PC」는 Windows 컴퓨터로 읽힌다 → 「컴퓨터」, 자기 자신은 「이 Mac」.
/// 상대 화면의 이름을 가리키는 곳(「이 PC 주소」 · 「상대 PC 주소」)은 상대가 Windows 일 수도 있어 두 이름을 함께 적는다.
const WORDS: &[(&str, &str)] = &[
    ("Windows 시작 시 실행", "로그인 시 열기"),
    (
        "Windows 에 로그인하면 LogiFlow 가 알림 영역에서 자동으로 시작합니다.",
        "Mac 에 로그인하면 LogiFlow 가 메뉴 막대에서 자동으로 시작합니다.",
    ),
    ("LogiFlow 가 설치된 폴더 안 logs 폴더에 있습니다.", "기록 파일이 든 폴더를 Finder 에서 엽니다."),
    (
        "시스템 설정을 따르거나 라이트·다크로 고정합니다.",
        "자동은 macOS 설정을 따르고, 라이트·다크는 고정합니다.",
    ),
    ("폴더 열기", "Finder 에서 보기"),
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

/// 글 전체가 이것일 때만(세그먼트 항목 등).
const EXACT: &[(&str, &str)] = &[("시스템", "자동")];

/// 상대 화면 이름(먼저 감춰 두었다가 마지막에 두 이름으로).
const PEER_LABELS: &[(&str, char, &str)] = &[
    ("「이 PC 주소」", '\u{E000}', "「이 PC 주소」(Mac 은 「이 Mac 주소」)"),
    ("「상대 PC 주소」", '\u{E001}', "「상대 PC 주소」(Mac 은 「상대 컴퓨터 주소」)"),
];

/// 낱말 「PC」(IPC 같은 낱말 속은 빼고) → 「컴퓨터」. 뒤에 띄어 쓴 조사는 붙인다(「PC 가」 → 「컴퓨터가」).
fn pc_to_computer(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 16);
    let mut i = 0;
    while i < chars.len() {
        let word = chars[i] == 'P'
            && chars.get(i + 1) == Some(&'C')
            && (i == 0 || !chars[i - 1].is_ascii_alphanumeric())
            && !chars.get(i + 2).map(|c| c.is_ascii_alphanumeric()).unwrap_or(false);
        if !word {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        out.push_str("컴퓨터");
        i += 2;
        if chars.get(i) == Some(&' ') {
            if let Some(&n) = chars.get(i + 1) {
                if "가는를로에의와도만".contains(n) {
                    i += 1; // 띄어 쓴 조사 → 붙여 씀
                }
            }
        }
    }
    out
}

pub fn localize(s: &str) -> Cow<'_, str> {
    if let Some((_, to)) = EXACT.iter().find(|(from, _)| *from == s) {
        return Cow::Owned(to.to_string());
    }
    if !s.contains("PC") && !WORDS.iter().any(|(from, _)| s.contains(from)) {
        return Cow::Borrowed(s);
    }
    let mut out = s.to_string();
    for (label, token, _) in PEER_LABELS {
        out = out.replace(label, &token.to_string());
    }
    for (from, to) in WORDS {
        if out.contains(from) {
            out = out.replace(from, to);
        }
    }
    let mut out = pc_to_computer(&out);
    for (_, token, full) in PEER_LABELS {
        out = out.replace(*token, full);
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::localize;

    #[test]
    fn mac_words() {
        assert_eq!(localize("이 PC 로 되돌리려면"), "이 Mac 으로 되돌리려면");
        assert_eq!(localize("상대 PC 주소"), "상대 컴퓨터 주소");
        assert_eq!(localize("두 PC 에 같은 값을 넣습니다."), "두 컴퓨터에 같은 값을 넣습니다.");
        assert_eq!(
            localize("상대 PC 의 「이 PC 주소」를 넣습니다."),
            "상대 컴퓨터의 「이 PC 주소」(Mac 은 「이 Mac 주소」)를 넣습니다."
        );
        assert_eq!(localize("IPC 창"), "IPC 창");
        assert_eq!(localize("시스템"), "자동");
        assert_eq!(localize("시스템 설정을 따릅니다"), "시스템 설정을 따릅니다");
        assert_eq!(localize("폴더 열기"), "Finder 에서 보기");
    }
}
