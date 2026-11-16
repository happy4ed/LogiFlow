//! 창·IPC 상대 번호 — Windows `wnd::Hwnd` 와 같은 모양. Mac 에서 `HWND` 는 설정 창의 NSView 주소이거나
//! IPC 상대(0 = 상주 앱, 그 밖 = 설정 창 프로세스 번호)를 담는 숫자일 뿐이다.

use super::dialog::HWND;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hwnd(pub isize);

impl Hwnd {
    pub fn of(h: HWND) -> Hwnd {
        Hwnd(h.0)
    }
    pub fn get(self) -> HWND {
        HWND(self.0)
    }
}
