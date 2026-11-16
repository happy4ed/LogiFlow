//! 설정 창(공용 egui 코드)이 쓰는 대화상자 상수·창 핸들 — OS 별 같은 이름(Mac 은 logiflow-mac::dialog).

pub use windows::Win32::Foundation::HWND;
pub use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_ICONWARNING, MB_OK, MB_YESNO, MESSAGEBOX_RESULT, MESSAGEBOX_STYLE,
};

/// 창 안 좌표(egui point) → 화면 좌표(메뉴 위치). Windows 메뉴는 물리 픽셀.
pub fn to_screen(h: Option<HWND>, x: f32, y: f32, pixels_per_point: f32) -> (i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    let mut origin = POINT::default();
    if let Some(h) = h {
        unsafe {
            let _ = ClientToScreen(h, &mut origin);
        }
    }
    (origin.x + (x * pixels_per_point) as i32, origin.y + (y * pixels_per_point) as i32)
}

/// 이 OS 에서 되는 기능(설정 창이 묶음을 보이거나 숨김).
pub struct Caps {
    pub prelogon: bool,
    pub probe: bool,
    pub firewall: bool,
    pub legacy: bool,
}

pub const CAPS: Caps = Caps {
    prelogon: true,
    probe: true,
    firewall: true,
    legacy: true,
};
