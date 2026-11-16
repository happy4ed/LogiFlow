//! LogiFlow macOS — 메뉴 막대 상주 앱과 설정 창(`--ui`)이 쓰는 OS 어댑터.
//!
//! Swift 판(`mac/` 0.15.0)의 IOKit HID 전송 · 가장자리 감시(NSEvent 전역 모니터 · 권한 불필요) · glow 를 옮기고,
//! Windows 판과 같은 설정 창(공용 egui `logiflow-ui`)이 부르는 함수 모양을 `logiflow-win` 과 같은 이름으로 갖춘다
//! (`dialog` · `util` · `ipc` · `notify` · `prompt` · `monitors` · `clipboard` …). Mac 에 없는 묶음(로그인 전에도 실행 ·
//! 로그인 화면 테스트 · Windows 방화벽 · 기존 LogiFlow 정리)은 빈 구현이고 설정 창이 `dialog::CAPS` 로 숨긴다.

pub mod migrate;

#[cfg(target_os = "macos")]
mod sys;
#[cfg(target_os = "macos")]
pub use sys::*;
