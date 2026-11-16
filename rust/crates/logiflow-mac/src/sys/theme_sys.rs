//! 시스템 다크 모드(설정 창 「시스템」 테마 · glow 색).

use objc2_foundation::{NSString, NSUserDefaults};

pub fn system_is_dark() -> bool {
    // 전역 도메인 AppleInterfaceStyle = "Dark"(자동 전환 중이면 지금 상태). 메인 스레드가 아니어도 읽을 수 있다.
    let d = NSUserDefaults::standardUserDefaults();
    match d.stringForKey(&NSString::from_str("AppleInterfaceStyle")) {
        Some(s) => s.to_string().eq_ignore_ascii_case("dark"),
        None => false,
    }
}

pub fn is_dark(mode: logiflow_core::config::ThemeMode) -> bool {
    use logiflow_core::config::ThemeMode;
    match mode {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        _ => system_is_dark(),
    }
}
