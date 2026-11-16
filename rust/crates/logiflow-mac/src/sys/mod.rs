//! macOS 구현(메뉴 막대 상주 앱 · 설정 창 공용).

pub mod autostart;
pub mod clipboard;
pub mod dialog;
pub mod edge_watcher;
pub mod ffi;
pub mod glow_mac;
pub mod hid;
pub mod ipc;
pub mod mainq;
pub mod monitors;
pub mod notify;
pub mod paths;
pub mod prompt;
pub mod session;
pub mod stubs;
pub mod theme_sys;
pub mod tray;
pub mod util;
pub mod wnd;

pub use stubs::{firewall, legacy, prelogon, probe};
