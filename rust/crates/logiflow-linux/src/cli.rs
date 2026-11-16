//! CLI — C# 리눅스 판(`linux/src/Program.cs`)과 같은 명령·같은 글.
//!   logiflow-rs switch <채널> · logiflow-rs <채널> · logiflow-rs list · --rescan · --debug · selftest · --version
//! 전환 쓰기는 공용 코어의 `switch_all_burst` 한 경로(가드 동일). 기기 정보는 캐시해 두고 바로 보낸다.

use crate::cache;
use crate::descriptor;
use crate::hidraw::{self, HidrawBackend, HidrawStream, OpenError};
use logiflow_core::log::DiagLogger;
use logiflow_core::scan::{self, HidInterfaceInfo, ScanResult, LOGITECH_VID};
use logiflow_core::switcher;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(args: &[String]) -> i32 {
    let mut debug = false;
    let mut rescan = false;
    let mut rest: Vec<&str> = Vec::new();
    for a in args {
        match a.as_str() {
            "--debug" => debug = true,
            "--rescan" => rescan = true,
            s => rest.push(s),
        }
    }
    let cmd = rest.first().copied().unwrap_or("");
    match cmd {
        "" => {
            usage();
            return 1;
        }
        "-h" | "--help" | "help" => {
            usage();
            return 0;
        }
        "--version" | "version" => {
            println!("logiflow-rs {}", VERSION);
            return 0;
        }
        "selftest" => return selftest(),
        _ => {}
    }
    let raw = DiagLogger::new(&cache::dir(), debug);
    let code = if cmd == "list" {
        list(&raw)
    } else if let Some(n) = (cmd == "switch")
        .then(|| rest.get(1).and_then(|s| channel(s)))
        .flatten()
        .or_else(|| channel(cmd))
    {
        switch(n, rescan, &raw)
    } else {
        usage();
        return 1;
    };
    if debug {
        println!("HID 로그: {}", raw.log_path().display());
    }
    code
}

fn usage() {
    println!("logiflow-rs {} — 로지텍 Easy-Switch 채널 전환(리눅스)", VERSION);
    println!();
    println!("  logiflow-rs switch <채널>   이 PC 에 붙은 마우스·키보드를 채널(1~3)로 넘김");
    println!("  logiflow-rs <채널>          위와 같음");
    println!("  logiflow-rs list            기기·현재 채널 보기(다시 찾음)");
    println!();
    println!("  --rescan   저장된 기기 정보 무시하고 다시 찾기");
    println!("  --debug    HID 송수신 로그 남기기");
    println!();
    println!("  기기가 지금 이 PC 에 연결돼 있을 때 다른 채널로 보내는 명령(당겨오기는 안 됨).");
}

fn channel(s: &str) -> Option<i32> {
    s.parse::<i32>().ok().filter(|n| (1..=6).contains(n))
}

fn switchable(all: &[ScanResult]) -> Vec<ScanResult> {
    scan::switchable(all)
}

fn identity(path: &str) -> Option<String> {
    hidraw::identity_of(path).filter(|s| s != "|")
}

/// HID++ 노드가 있는지·열 권한이 있는지 먼저 본다(없으면 안내하고 끝).
fn precheck(nodes: &[HidInterfaceInfo]) -> i32 {
    let (mut any, mut denied) = (false, false);
    for info in nodes.iter().filter(|i| i.output_report_len >= 7) {
        any = true;
        if let Err(OpenError::Denied) = HidrawStream::open(info) {
            denied = true;
        }
    }
    if !any {
        eprintln!("로지텍 HID++ 장치 없음 — 리시버(Bolt/Unifying)나 블루투스로 이 PC 에 연결돼 있어야 함");
        return 1;
    }
    if denied {
        eprintln!("장치 권한 없음 — 한 번만: sudo ./install.sh (udev 규칙 설치) 후 다시 실행. 급하면 sudo logiflow-rs …");
        return 2;
    }
    0
}

fn scan_and_save(raw: &DiagLogger) -> Vec<ScanResult> {
    let list = switchable(&scan::scan(&HidrawBackend, raw));
    if !list.is_empty() {
        cache::save(&list, &identity);
    }
    list
}

fn load_cache(nodes: &[HidInterfaceInfo]) -> Vec<ScanResult> {
    let keyed: Vec<(String, HidInterfaceInfo)> = nodes.iter().filter_map(|i| identity(&i.path).map(|id| (id, i.clone()))).collect();
    cache::load(&keyed)
}

fn switch(channel: i32, rescan: bool, raw: &DiagLogger) -> i32 {
    let nodes = hidraw::enumerate(LOGITECH_VID, 0);
    let pre = precheck(&nodes);
    if pre != 0 {
        return pre;
    }
    let mut devs = if rescan { Vec::new() } else { load_cache(&nodes) };
    let from_cache = !devs.is_empty();
    if !from_cache {
        println!("기기 찾는 중…");
        devs = scan_and_save(raw);
    }
    if devs.is_empty() {
        eprintln!("전환할 기기 없음 — 마우스·키보드가 지금 이 PC 에 연결돼 있어야 함");
        return 1;
    }
    let mut ok = run_switch(&devs, channel, raw);
    if ok == 0 && from_cache {
        // 저장된 정보가 낡았을 수 있음(재페어링·다른 리시버) → 다시 찾아 한 번 더.
        println!("저장된 기기 정보로 실패 — 다시 찾는 중…");
        devs = scan_and_save(raw);
        if !devs.is_empty() {
            ok = run_switch(&devs, channel, raw);
        }
    }
    if ok > 0 {
        0
    } else {
        1
    }
}

fn run_switch(devs: &[ScanResult], channel: i32, raw: &DiagLogger) -> usize {
    println!("채널 {} 으로 전환 — 기기 {}개", channel, devs.len());
    let ok = switcher::switch_all_burst(&HidrawBackend, devs, channel - 1, raw, &mut |s| println!("{}", s), 3, None);
    println!("결과: {}/{}개 전환", ok, devs.len());
    ok
}

fn list(raw: &DiagLogger) -> i32 {
    let nodes = hidraw::enumerate(LOGITECH_VID, 0);
    let pre = precheck(&nodes);
    if pre != 0 {
        return pre;
    }
    println!("기기 찾는 중…");
    let all = scan::scan(&HidrawBackend, raw);
    cache::save(&switchable(&all), &identity);
    if all.is_empty() {
        println!("응답한 기기 없음 — 마우스·키보드가 지금 이 PC 에 연결돼 있는지 확인");
        return 1;
    }
    for r in &all {
        let ch = if r.change_host_supported {
            if r.host_count > 0 {
                format!(
                    "채널 {}개 · 지금 {}(이 PC)",
                    r.host_count,
                    if r.current_host >= 0 { (r.current_host + 1).to_string() } else { "?".into() }
                )
            } else {
                "채널 정보 없음".into()
            }
        } else {
            "Easy-Switch 미지원".into()
        };
        println!("  {} — {} · {} · {}", r.device_name, r.connection_type, ch, r.path);
    }
    0
}

/// CI 확인용 — 서술자 해석이 리시버·블루투스·일반 마우스를 맞게 가르는지, 이 머신 열거가 끝나는지.
fn selftest() -> i32 {
    let mut fail = 0;
    let mut check = |label: &str, d: &str, page: u16, usage: u16, len: usize| {
        let c = descriptor::parse(&descriptor::hex(d));
        let ok = c.usage_page == page && c.usage == usage && c.report_len == len;
        println!(
            "{}: page=0x{:04X} usage=0x{:04X} out={} → {}",
            label,
            c.usage_page,
            c.usage,
            c.report_len,
            if ok { "OK" } else { "FAIL" }
        );
        if !ok {
            fail += 1;
        }
    };
    check("receiver", descriptor::RECEIVER, 0xFF00, 0x0202, 20);
    check("bluetooth", descriptor::BLUETOOTH, 0xFF43, 0x0202, 20);
    check("mouse", descriptor::MOUSE, 0, 0, 0);
    println!("enumerate: 로지텍 노드 {}개 → OK", hidraw::enumerate(LOGITECH_VID, 0).len());
    if fail == 0 {
        println!("selftest OK");
        0
    } else {
        println!("selftest FAIL {}", fail);
        1
    }
}
