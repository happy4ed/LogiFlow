//! LogiFlow 코어 — OS 와 무관한 부분(HID++ 0x1814 · 스캔 · 전환 안전 · 가장자리 판정 · 설정/캐시 형식 ·
//! 클립보드 암호 · glow 계산 · 테마 토큰). C# 판(0.15.0) 동작을 그대로 옮겼다.

pub mod cache;
pub mod clip;
pub mod config;
pub mod edge;
pub mod glow;
pub mod hidpp;
pub mod log;
pub mod logon_edges;
pub mod scan;
pub mod switcher;
pub mod theme;

#[cfg(test)]
mod tests;
