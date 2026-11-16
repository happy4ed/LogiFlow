//! LogiFlow 리눅스 — hidraw 전송·열거와 CLI. HID++ 프로토콜·스캔·전환 가드는 공용 코어(`logiflow-core`) 그대로.
//! 서술자 해석·캐시 형식은 OS 무관(어디서나 시험), hidraw·CLI 는 리눅스에서만 빌드된다.

pub mod cache;
#[cfg(target_os = "linux")]
pub mod cli;
pub mod descriptor;
#[cfg(target_os = "linux")]
pub mod hidraw;
