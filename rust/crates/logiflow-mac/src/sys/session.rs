//! 시각 문자열(설정 창 시험 글·기록 표시).

/// 지금 시각 `HH:MM:SS`.
pub fn local_time_hms() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}
