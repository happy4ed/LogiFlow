//! 진단 로그 — C# `DiagLogger` 이식. 보낸 요청/받은 응답 raw 바이트(hex)·타임아웃·오류를
//! `<base>/logs/logiflow-diag.log` 에 남긴다. serial 등 민감값은 기록하지 않는다.
//! C# 판의 메모리 사본(Snapshot)은 어디서도 읽지 않아 옮기지 않았다(상주 메모리 증가 방지).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub struct DiagLogger {
    dir: PathBuf,
    path: PathBuf,
    enabled: AtomicBool,
    lock: Mutex<()>,
}

pub fn newline() -> &'static str {
    if cfg!(windows) {
        "\r\n"
    } else {
        "\n"
    }
}

impl DiagLogger {
    pub fn new(base_dir: &Path, enabled: bool) -> Self {
        let dir = base_dir.join("logs");
        let path = dir.join("logiflow-diag.log");
        DiagLogger {
            dir,
            path,
            enabled: AtomicBool::new(enabled),
            lock: Mutex::new(()),
        }
    }

    /// 파일 기록 없는 로거(조회 전용 스캔 등).
    pub fn null() -> Self {
        DiagLogger {
            dir: PathBuf::new(),
            path: PathBuf::new(),
            enabled: AtomicBool::new(false),
            lock: Mutex::new(()),
        }
    }

    pub fn log_directory(&self) -> &Path {
        &self.dir
    }
    pub fn log_path(&self) -> &Path {
        &self.path
    }
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed)
    }

    pub fn hex(data: &[u8]) -> String {
        let mut s = String::with_capacity(data.len() * 3);
        for (i, b) in data.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&format!("{:02X}", b));
        }
        s
    }

    pub fn log(&self, line: &str) {
        if !self.enabled() || self.path.as_os_str().is_empty() {
            return;
        }
        let stamped = format!("{}  {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), line);
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        append_utf8(&self.dir, &self.path, &format!("{}{}", stamped, newline()));
    }

    pub fn tx(&self, device_index: u8, report: &[u8]) {
        self.log(&format!("TX  dev=0x{:02X}  [{}]", device_index, Self::hex(report)));
    }
    pub fn rx(&self, report: &[u8]) {
        self.log(&format!("RX  [{}]", Self::hex(report)));
    }
    pub fn timeout(&self, context: &str) {
        self.log(&format!("TIMEOUT  {}", context));
    }
    pub fn error(&self, context: &str) {
        self.log(&format!("ERROR  {}", context));
    }
}

/// UTF-8 덧붙여 쓰기. 새 파일이면 BOM 을 먼저 쓴다(C# File.AppendAllText(…, Encoding.UTF8) 과 같은 모양).
pub fn append_utf8(dir: &Path, path: &Path, text: &str) {
    let _ = fs::create_dir_all(dir);
    let is_new = fs::metadata(path).map(|m| m.len() == 0).unwrap_or(true);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        if is_new {
            let _ = f.write_all(&[0xEF, 0xBB, 0xBF]);
        }
        let _ = f.write_all(text.as_bytes());
    }
}

/// UTF-8(BOM) 통째 쓰기 — C# File.WriteAllText(…, Encoding.UTF8) 과 같은 모양.
pub fn write_utf8_bom(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    let mut bytes = Vec::with_capacity(text.len() + 3);
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(text.as_bytes());
    fs::write(path, bytes)
}

/// 파일을 줄 단위로 읽는다(BOM·CRLF 제거). C# File.ReadAllLines 대응.
pub fn read_lines(path: &Path) -> Option<Vec<String>> {
    let bytes = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    Some(text.lines().map(|l| l.trim_end_matches('\r').to_string()).collect())
}
