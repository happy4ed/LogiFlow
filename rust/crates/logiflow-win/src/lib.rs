//! LogiFlow Windows 어댑터 — C# 판(0.15.0) Windows 전용 부분 이식.
#![cfg(windows)]

pub mod autostart;
pub mod clipboard;
pub mod dpapi;
pub mod edge_watcher;
pub mod firewall;
pub mod glow_win;
pub mod hid;
pub mod ipc;
pub mod legacy;
pub mod monitors;
pub mod notify;
pub mod paths;
pub mod prelogon;
pub mod probe;
pub mod prompt;
pub mod rawinput;
pub mod service;
pub mod service_ctl;
pub mod session;
pub mod theme_sys;
pub mod tray;
pub mod util;
pub mod wnd;
pub mod workers;
