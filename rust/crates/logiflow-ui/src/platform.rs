//! OS 별 구현을 같은 이름으로 — 설정 창 코드(app.rs)는 `sys::…` 만 쓴다.
//! Windows = logiflow-win(트레이와 별도 프로세스 · IPC), Mac = logiflow-mac(같은 프로세스).

#[cfg(windows)]
pub use logiflow_win as sys;

#[cfg(target_os = "macos")]
pub use logiflow_mac as sys;
