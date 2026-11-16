//! 두 컴퓨터 간 클립보드 공유(텍스트·이미지) — Windows `clipboard` 의 Mac 판. 프로토콜은 코어 `clip`
//! (C# 판 · Windows 러스트 판과 바이트 호환: 길이 4 LE + IV16 + AES-256-CBC + HMAC-SHA256 32).
//! 변화 감지는 NSPasteboard changeCount 를 0.4초마다 보고(macOS 에는 변화 알림이 없다), 읽기·쓰기는 메인 스레드,
//! 네트워크는 백그라운드.

use super::mainq;
use super::util::tick_ms;
use logiflow_core::clip::{self, ClipKeys, KIND_TEXT, MAX_BYTES};
use logiflow_core::config::AppConfig;
use logiflow_core::log::DiagLogger;
use objc2_app_kit::NSPasteboard;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 수신 적용(설명 문자열) · 인증 실패 알림.
pub struct ClipEvents {
    pub on_received: Rc<dyn Fn(&str)>,
    pub on_auth_fail: Rc<dyn Fn()>,
}

struct Sync {
    peer: String,
    port: i32,
    keys: Arc<ClipKeys>,
    running: Arc<AtomicBool>,
    last_hash: Arc<Mutex<String>>,
    applied_at: u64,
    change_count: isize,
    log: Arc<DiagLogger>,
    accept: Option<std::thread::JoinHandle<()>>,
    timer: Option<core_foundation_sys::runloop::CFRunLoopTimerRef>,
}

thread_local! {
    static S: RefCell<Option<Sync>> = const { RefCell::new(None) };
    static EVENTS: RefCell<Option<ClipEvents>> = const { RefCell::new(None) };
}

pub fn set_events(ev: ClipEvents) {
    EVENTS.with(|e| *e.borrow_mut() = Some(ev));
}

pub fn running() -> bool {
    S.with(|s| s.borrow().as_ref().map(|s| s.running.load(Ordering::SeqCst)).unwrap_or(false))
}

fn change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
}

/// 설정 적용(메인 스레드). 기존 것 멈추고 새로 시작.
pub fn apply(cfg: &AppConfig, log: &Arc<DiagLogger>) {
    stop();
    if !cfg.clip_enabled {
        return;
    }
    if cfg.clip_secret.is_empty() {
        log.log("클립보드 공유: 암호문구 없음 — 비활성");
        return;
    }
    if cfg.clip_peer_host.is_empty() {
        log.log("클립보드 공유: 상대 주소 없음 — 비활성");
        return;
    }
    let peer = clip::norm(&cfg.clip_peer_host);
    let port = cfg.clip_port;
    let keys = Arc::new(clip::derive_keys(&cfg.clip_secret));
    let listener = match TcpListener::bind(("0.0.0.0", port as u16)) {
        Ok(l) => l,
        Err(e) => {
            log.error(&format!("클립보드 공유 시작 실패(포트 사용중?): {}", e));
            return;
        }
    };
    let running = Arc::new(AtomicBool::new(true));
    let last_hash = Arc::new(Mutex::new(String::new()));
    let accept = {
        let (running, keys, log, last_hash) = (running.clone(), keys.clone(), log.clone(), last_hash.clone());
        std::thread::Builder::new()
            .name("LogiFlow.Clip.Accept".into())
            .spawn(move || accept_loop(listener, running, keys, log, last_hash))
            .ok()
    };
    log.log(&format!("클립보드 공유 시작: 수신 포트 {} · 상대 {}:{}", port, peer, port));
    let timer = unsafe {
        use core_foundation_sys::runloop::*;
        let t = CFRunLoopTimerCreate(
            core_foundation_sys::base::kCFAllocatorDefault,
            core_foundation_sys::date::CFAbsoluteTimeGetCurrent() + 0.4,
            0.4,
            0,
            0,
            on_poll,
            std::ptr::null_mut(),
        );
        CFRunLoopAddTimer(CFRunLoopGetMain(), t, kCFRunLoopCommonModes);
        t
    };
    S.with(|s| {
        *s.borrow_mut() = Some(Sync {
            peer,
            port,
            keys,
            running,
            last_hash,
            applied_at: 0,
            change_count: change_count(),
            log: log.clone(),
            accept,
            timer: Some(timer),
        })
    });
}

pub fn stop() {
    if let Some(mut s) = S.with(|s| s.borrow_mut().take()) {
        s.running.store(false, Ordering::SeqCst);
        if let Some(t) = s.timer.take() {
            unsafe {
                core_foundation_sys::runloop::CFRunLoopTimerInvalidate(t);
                core_foundation_sys::base::CFRelease(t as _);
            }
        }
        // 막혀 있는 accept 를 깨워 수신 스레드가 끝나게(자기 포트로 한 번 연결).
        let _ = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], s.port as u16)), Duration::from_millis(300));
        if let Some(h) = s.accept.take() {
            let until = Instant::now() + Duration::from_millis(2000);
            while !h.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

extern "C" fn on_poll(_t: core_foundation_sys::runloop::CFRunLoopTimerRef, _i: *mut std::ffi::c_void) {
    let changed = S.with(|s| {
        let mut b = s.borrow_mut();
        let s = b.as_mut()?;
        let now = change_count();
        if now == s.change_count {
            return None;
        }
        s.change_count = now;
        Some(())
    });
    if changed.is_some() {
        on_clipboard_update();
    }
}

fn read_clipboard(log: &DiagLogger) -> Option<Vec<u8>> {
    let mut cb = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            log.error(&format!("클립보드 읽기 실패: {}", e));
            return None;
        }
    };
    if let Ok(t) = cb.get_text() {
        return Some(clip::text_payload(&t));
    }
    if let Ok(img) = cb.get_image() {
        match encode_png(img.width as u32, img.height as u32, &img.bytes) {
            Some(png) if png.len() <= MAX_BYTES => return Some(clip::image_payload(&png)),
            Some(_) => log.log("클립보드 이미지 8MB 초과 — 전송 안 함"),
            None => log.error("클립보드 읽기 실패: PNG 변환"),
        }
    }
    None
}

fn on_clipboard_update() {
    let job = S.with(|s| {
        let mut b = s.borrow_mut();
        let s = b.as_mut()?;
        // 방금 수신 적용으로 생긴 내 클립보드 변화는 되돌려 보내지 않는다(이미지 재인코딩 핑퐁 차단).
        if tick_ms().saturating_sub(s.applied_at) < 1200 {
            return None;
        }
        let payload = read_clipboard(&s.log)?;
        let h = clip::hash(&payload);
        {
            let mut lh = s.last_hash.lock().ok()?;
            if *lh == h {
                return None;
            }
            *lh = h;
        }
        s.log.log(&format!(
            "클립보드 변화 감지 → 전송 시도({} {}바이트, 상대 {}:{})",
            if payload[0] == KIND_TEXT { "텍스트" } else { "이미지" },
            payload.len(),
            s.peer,
            s.port
        ));
        let frame = clip::encrypt(&s.keys, &payload);
        Some((frame, s.peer.clone(), s.port, s.log.clone()))
    });
    if let Some((frame, peer, port, log)) = job {
        std::thread::Builder::new()
            .name("LogiFlow.Clip.Send".into())
            .spawn(move || match send_frame(&peer, port, &frame) {
                Ok(()) => log.log(&format!("클립보드 전송 성공 → {}", peer)),
                Err(SendErr::Timeout) => log.error(&format!("클립보드 전송: 상대 연결 시간초과 {}", peer)),
                Err(SendErr::Other(e)) => log.error(&format!("클립보드 전송 실패({}): {}", peer, e)),
            })
            .ok();
    }
}

pub enum SendErr {
    Timeout,
    Other(String),
}

/// host:port 로 3초 안에 연결(이름이면 모든 주소 시도).
pub fn connect(host: &str, port: i32) -> Result<TcpStream, SendErr> {
    let addrs: Vec<SocketAddr> = (host, port as u16).to_socket_addrs().map_err(|e| SendErr::Other(e.to_string()))?.collect();
    if addrs.is_empty() {
        return Err(SendErr::Other("주소를 찾지 못함".into()));
    }
    let deadline = Instant::now() + Duration::from_millis(3000);
    let mut last = SendErr::Timeout;
    for a in addrs {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(SendErr::Timeout);
        }
        match TcpStream::connect_timeout(&a, left) {
            Ok(s) => return Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => last = SendErr::Timeout,
            Err(e) => last = SendErr::Other(e.to_string()),
        }
    }
    Err(last)
}

pub fn send_frame(host: &str, port: i32, frame: &[u8]) -> Result<(), SendErr> {
    let mut s = connect(host, port)?;
    s.write_all(&clip::wire(frame)).map_err(|e| SendErr::Other(e.to_string()))?;
    s.flush().map_err(|e| SendErr::Other(e.to_string()))
}

fn read_exact(s: &mut TcpStream, n: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; n];
    s.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn accept_loop(listener: TcpListener, running: Arc<AtomicBool>, keys: Arc<ClipKeys>, log: Arc<DiagLogger>, last_hash: Arc<Mutex<String>>) {
    while running.load(Ordering::SeqCst) {
        let (mut cli, _) = match listener.accept() {
            Ok(c) => c,
            Err(_) => break,
        };
        if !running.load(Ordering::SeqCst) {
            break;
        }
        let _ = cli.set_read_timeout(Some(Duration::from_millis(5000)));
        let Some(lenb) = read_exact(&mut cli, 4) else {
            continue;
        };
        let len = i32::from_le_bytes([lenb[0], lenb[1], lenb[2], lenb[3]]);
        if !clip::valid_len(len) {
            continue;
        }
        let Some(frame) = read_exact(&mut cli, len as usize) else {
            continue;
        };
        log.log(&format!("클립보드 프레임 수신 {}바이트", len));
        match clip::decrypt(&keys, &frame) {
            Some(p) if !p.is_empty() => {
                let h = clip::hash(&p);
                let (log2, lh) = (log.clone(), last_hash.clone());
                mainq::post(move || apply_payload(p, h, log2, lh));
            }
            _ => {
                log.error("클립보드 수신 인증 실패(암호문구 불일치?)");
                mainq::post(|| {
                    EVENTS.with(|e| {
                        if let Some(ev) = e.borrow().as_ref() {
                            (ev.on_auth_fail)();
                        }
                    })
                });
            }
        }
    }
}

fn apply_payload(payload: Vec<u8>, hash: String, log: Arc<DiagLogger>, last_hash: Arc<Mutex<String>>) {
    if let Ok(mut lh) = last_hash.lock() {
        *lh = hash; // 우리가 세팅 → 뒤따르는 변화 감지 무시
    }
    let mut cb = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            log.error(&format!("클립보드 적용 실패: {}", e));
            return;
        }
    };
    let desc;
    if payload[0] == KIND_TEXT {
        let t = String::from_utf8_lossy(&payload[1..]).to_string();
        let r = if t.is_empty() { cb.clear() } else { cb.set_text(t.clone()) };
        if let Err(e) = r {
            log.error(&format!("클립보드 적용 실패: {}", e));
            return;
        }
        desc = format!("텍스트 {}자", t.encode_utf16().count());
        log.log(&format!("클립보드 수신 적용({})", desc));
    } else {
        match decode_png(&payload[1..]) {
            Some((w, h, rgba)) => {
                let img = arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: rgba.into(),
                };
                if let Err(e) = cb.set_image(img) {
                    log.error(&format!("클립보드 적용 실패: {}", e));
                    return;
                }
            }
            None => {
                log.error("클립보드 적용 실패: 이미지 해석 불가");
                return;
            }
        }
        desc = "이미지".to_string();
        log.log("클립보드 수신 적용(이미지)");
    }
    // 적용으로 바뀐 changeCount 는 내 변화가 아니다 — 다음 폴링이 다시 보내지 않게 지금 값으로 맞춘다.
    S.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.applied_at = tick_ms();
            s.change_count = change_count();
        }
    });
    EVENTS.with(|e| {
        if let Some(ev) = e.borrow().as_ref() {
            (ev.on_received)(&desc);
        }
    });
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().ok()?;
        wr.write_image_data(rgba).ok()?;
    }
    Some(out)
}

fn decode_png(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(data));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let px = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        _ => return None,
    };
    Some((w, h, rgba))
}

// ---- 설정 창 진단 보조 ----

/// 이 Mac 의 IPv4 주소 하나와 그 네트워크.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalAddr {
    pub ip: String,
    /// 쉬운 말 네트워크 이름(「유선 랜(Ethernet)」·「와이파이(Wi-Fi)」·「가상 어댑터(bridge100)」).
    pub network: String,
    /// 가상 머신·인터넷 공유 브리지·VPN 등 — 다른 컴퓨터와 연결할 때는 보통 쓰지 않는다.
    pub virtual_adapter: bool,
}

/// 인터페이스 이름(en0) → 하드웨어 포트 이름(Wi-Fi · Ethernet · Thunderbolt Bridge …). `networksetup` 으로 한 번.
fn hardware_ports() -> Vec<(String, String)> {
    let Ok(out) = std::process::Command::new("/usr/sbin/networksetup").arg("-listallhardwareports").output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut v = Vec::new();
    let mut port: Option<String> = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("Hardware Port: ") {
            port = Some(p.trim().to_string());
        } else if let Some(d) = line.strip_prefix("Device: ") {
            if let Some(p) = port.take() {
                v.push((d.trim().to_string(), p));
            }
        }
    }
    v
}

/// 켜져 있는 인터페이스의 IPv4 주소(네트워크 이름 포함). 실제 랜·와이파이 먼저, 가상 어댑터는 뒤.
pub fn local_addresses() -> Vec<LocalAddr> {
    let ports = hardware_ports();
    let mut out: Vec<LocalAddr> = Vec::new();
    unsafe {
        let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut head) != 0 {
            return out;
        }
        let mut p = head;
        while !p.is_null() {
            let a = &*p;
            p = a.ifa_next;
            if a.ifa_addr.is_null() || (*a.ifa_addr).sa_family as i32 != libc::AF_INET {
                continue;
            }
            let up = (a.ifa_flags & libc::IFF_UP as u32) != 0 && (a.ifa_flags & libc::IFF_RUNNING as u32) != 0;
            if !up {
                continue;
            }
            let sin = &*(a.ifa_addr as *const libc::sockaddr_in);
            let ip = std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
            if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() {
                continue;
            }
            let name = std::ffi::CStr::from_ptr(a.ifa_name).to_string_lossy().to_string();
            let port = ports.iter().find(|(d, _)| *d == name).map(|(_, p)| p.clone());
            let virtual_adapter = [
                "bridge", "utun", "vmnet", "vboxnet", "llw", "awdl", "gif", "stf", "anpi", "ipsec", "ppp", "feth", "vnic",
            ]
            .iter()
            .any(|pre| name.starts_with(pre));
            let network = match &port {
                _ if virtual_adapter => format!("가상 어댑터({})", name),
                Some(p) if p.contains("Wi-Fi") || p.contains("AirPort") => format!("와이파이({})", p),
                Some(p) if p.contains("Ethernet") || p.contains("LAN") => format!("유선 랜({})", p),
                Some(p) => p.clone(),
                None => name.clone(),
            };
            let ip = ip.to_string();
            if !out.iter().any(|x| x.ip == ip) {
                out.push(LocalAddr { ip, network, virtual_adapter });
            }
        }
        libc::freeifaddrs(head);
    }
    let rank = |a: &LocalAddr| {
        let o: Vec<u8> = a.ip.split('.').filter_map(|x| x.parse().ok()).collect();
        let lan = match o.as_slice() {
            [192, 168, ..] => 0,
            [10, ..] => 1,
            [172, b, ..] if (16..=31).contains(b) => 2,
            _ => 3,
        };
        (a.virtual_adapter as u8, lan)
    };
    out.sort_by_key(rank);
    out
}

/// 상대 컴퓨터에 TCP 연결 시도(도달성 확인). (연결됨, 안내 글).
pub fn test_connect(host: &str, port: i32) -> (bool, String) {
    let host = clip::norm(host);
    if host.is_empty() {
        return (false, "상대 컴퓨터 주소가 비어 있습니다.".into());
    }
    let r: String = match connect(&host, port) {
        Ok(_) => return (true, format!("상대 컴퓨터({}, 포트 {})에 연결됩니다.", host, port)),
        Err(SendErr::Timeout) => {
            "상대 컴퓨터에 연결하지 못했습니다(시간 초과).\n상대 컴퓨터가 켜져 있는지, 주소가 맞는지, 상대 컴퓨터의 방화벽이 허용됐는지 확인하세요.".into()
        }
        Err(SendErr::Other(e)) => format!("상대 컴퓨터에 연결하지 못했습니다.\n({})", e),
    };
    (false, r)
}

/// 상대에게 시험 글을 보낸다(종단 확인). running = 상주 앱 쪽 공유가 켜져 있는지. (보냄, 안내 글).
pub fn send_test(running: bool, cfg: &AppConfig) -> (bool, String) {
    if !running || cfg.clip_secret.is_empty() {
        return (false, "먼저 클립보드 공유를 켜고 상대 컴퓨터 주소와 암호문구를 넣어 주세요.".into());
    }
    let keys = clip::derive_keys(&cfg.clip_secret);
    let msg = format!("LogiFlow 연결 확인 ✓ {}", super::session::local_time_hms().get(..8).unwrap_or(""));
    let frame = clip::encrypt(&keys, &clip::text_payload(&msg));
    let host = clip::norm(&cfg.clip_peer_host);
    match send_frame(&host, cfg.clip_port, &frame) {
        Ok(()) => (
            true,
            format!(
                "상대 컴퓨터에 연결되어 시험 글을 보냈습니다.\n상대 컴퓨터에서 붙여 넣기(Windows 는 Ctrl+V · Mac 은 ⌘V) 했을 때\n\"{}\"\n가 나오면 양쪽이 같은 암호문구입니다.",
                msg
            ),
        ),
        Err(SendErr::Timeout) => (false, "보내지 못했습니다(시간 초과).\n상대 컴퓨터가 켜져 있는지, 상대 컴퓨터의 방화벽이 허용됐는지 확인하세요.".into()),
        Err(SendErr::Other(e)) => (false, format!("보내지 못했습니다.\n({})", e)),
    }
}
