//! `logiflow-rs` — 리눅스 CLI(C# 판 `logiflow` 와 같은 명령). 나란히 설치되게 이름만 다르다.
#[cfg(target_os = "linux")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(logiflow_linux::cli::run(&args));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("logiflow-rs 는 리눅스용입니다.");
    std::process::exit(1);
}
