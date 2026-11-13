//! 두 PC 간 클립보드 공유(텍스트·이미지) — C# `ClipboardSync` 이식. 프로토콜은 코어 `clip`(바이트 호환).
//! 클립보드 읽기/쓰기는 UI 스레드에서, 네트워크는 백그라운드에서.

use crate::util::tick_ms;
use crate::wnd::{self, Handler};
use logiflow_core::clip::{self, ClipKeys, KIND_TEXT, MAX_BYTES};
use logiflow_core::config::AppConfig;
use logiflow_core::log::DiagLogger;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LRESULT};
use windows::Win32::System::DataExchange::{AddClipboardFormatListener, RemoveClipboardFormatListener};
use windows::Win32::UI::WindowsAndMessaging::DestroyWindow;

pub const WM_CLIPBOARDUPDATE: u32 = 0x031D;

/// 수신 적용(설명 문자열) · 인증 실패 알림 — 트레이 풍선.
pub struct ClipEvents {
    pub on_received: Rc<dyn Fn(&str)>,
    pub on_auth_fail: Rc<dyn Fn()>,
}

struct Sync {
    hwnd: HWND,
    peer: String,
    port: i32,
    keys: Arc<ClipKeys>,
    running: Arc<AtomicBool>,
    last_hash: Arc<Mutex<String>>,
    applied_at: u64,
    log: Arc<DiagLogger>,
    accept: Option<std::thread::JoinHandle<()>>,
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

/// 설정 적용(UI 스레드). 기존 것 멈추고 새로 시작.
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
            log.error(&format!("클립보드 공유 시작 실패(포트 사용중/방화벽?): {}", e));
            return;
        }
    };
    let handler: Handler = Rc::new(|_h, msg, _w, _l| {
        if msg == WM_CLIPBOARDUPDATE {
            on_clipboard_update();
            return Some(LRESULT(0));
        }
        None
    });
    let hwnd = match wnd::message_window("LogiFlowRs.Clipboard", handler) {
        Some(h) => h,
        None => return,
    };
    unsafe {
        let _ = AddClipboardFormatListener(hwnd);
    }
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
    S.with(|s| {
        *s.borrow_mut() = Some(Sync {
            hwnd,
            peer,
            port,
            keys,
            running,
            last_hash,
            applied_at: 0,
            log: log.clone(),
            accept,
        })
    });
}

pub fn stop() {
    if let Some(s) = S.with(|s| s.borrow_mut().take()) {
        s.running.store(false, Ordering::SeqCst);
        unsafe {
            let _ = RemoveClipboardFormatListener(s.hwnd);
            let _ = DestroyWindow(s.hwnd);
        }
        // 막혀 있는 accept 를 깨워 수신 스레드가 끝나게(자기 포트로 한 번 연결).
        let _ = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], s.port as u16)), Duration::from_millis(300));
        // C# 은 _listener.Stop() 으로 바로 닫는다. 여기선 수신 스레드가 리스너를 쥐고 있으므로 끝날 때까지
        // 잠깐 기다린다 — 안 기다리면 곧바로 이어지는 apply() 의 bind 가 「포트 사용중」으로 실패해
        // 설정을 바꿀 때마다 공유가 꺼질 수 있다(Windows 는 SO_REUSEADDR 없음). 상한 2초.
        if let Some(h) = s.accept {
            let until = Instant::now() + Duration::from_millis(2000);
            while !h.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
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

/// host:port 로 3초 안에 연결(이름이면 모든 주소 시도 — C# TcpClient.BeginConnect 와 같게).
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
        let lenb = match read_exact(&mut cli, 4) {
            Some(b) => b,
            None => continue,
        };
        let len = i32::from_le_bytes([lenb[0], lenb[1], lenb[2], lenb[3]]);
        if !clip::valid_len(len) {
            continue;
        }
        let frame = match read_exact(&mut cli, len as usize) {
            Some(f) => f,
            None => continue,
        };
        log.log(&format!("클립보드 프레임 수신 {}바이트", len));
        match clip::decrypt(&keys, &frame) {
            Some(p) if !p.is_empty() => {
                let h = clip::hash(&p);
                let (log2, lh) = (log.clone(), last_hash.clone());
                wnd::post(move || apply_payload(p, h, log2, lh));
            }
            _ => {
                log.error("클립보드 수신 인증 실패(암호문구 불일치?)");
                wnd::post(|| {
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
    S.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.applied_at = tick_ms(); // 적용 직후 생길 clipboard-update 억제
        }
    });
    if let Ok(mut lh) = last_hash.lock() {
        *lh = hash; // 우리가 세팅 → 뒤따르는 clipboard-update 무시
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

/// 이 PC 의 IPv4 주소들 + 포트(상대 PC 에 입력할 값). C#: Dns.GetHostAddresses(Dns.GetHostName()).
pub fn local_info(port: i32) -> String {
    let host = computer_dns_name();
    let mut ips: Vec<String> = Vec::new();
    if let Ok(it) = (host.as_str(), 0u16).to_socket_addrs() {
        for a in it {
            if let SocketAddr::V4(v4) = a {
                let s = v4.ip().to_string();
                if !ips.contains(&s) {
                    ips.push(s);
                }
            }
        }
    }
    format!("{} : {}", if ips.is_empty() { "?".to_string() } else { ips.join(", ") }, port)
}

fn computer_dns_name() -> String {
    use windows::core::PWSTR;
    use windows::Win32::System::SystemInformation::{ComputerNameDnsHostname, GetComputerNameExW};
    let mut buf = [0u16; 256];
    let mut n = buf.len() as u32;
    unsafe {
        if GetComputerNameExW(ComputerNameDnsHostname, Some(PWSTR(buf.as_mut_ptr())), &mut n).is_ok() {
            return String::from_utf16_lossy(&buf[..n as usize]);
        }
    }
    "localhost".into()
}

/// 상대 PC 에 TCP 연결 시도(도달성 확인).
pub fn test_connect(host: &str, port: i32) -> String {
    let host = clip::norm(host);
    if host.is_empty() {
        return "상대 주소가 비어 있습니다.".into();
    }
    match connect(&host, port) {
        Ok(_) => format!("연결 OK — 상대 {}:{} 에 닿음.", host, port),
        Err(SendErr::Timeout) => "실패: 연결 시간초과 — 방화벽/주소/포트 확인.".into(),
        Err(SendErr::Other(e)) => format!("실패: {}", e),
    }
}

/// 상대에게 테스트 텍스트를 보낸다(종단 확인). running = 트레이 쪽 공유가 켜져 있는지.
pub fn send_test(running: bool, cfg: &AppConfig) -> String {
    if !running || cfg.clip_secret.is_empty() {
        return "먼저 '사용'을 켜고 상대 주소·암호문구를 설정하세요.".into();
    }
    let keys = clip::derive_keys(&cfg.clip_secret);
    let msg = format!("LogiFlow 클립보드 연결 테스트 ✓ {}", crate::session::local_time_hms().get(..8).unwrap_or(""));
    let frame = clip::encrypt(&keys, &clip::text_payload(&msg));
    let host = clip::norm(&cfg.clip_peer_host);
    match send_frame(&host, cfg.clip_port, &frame) {
        Ok(()) => format!("보냄 OK — 상대 PC 에서 붙여넣기(Ctrl+V) 해서\r\n\"{}\"\r\n가 나오면 양방향 성공입니다.", msg),
        Err(SendErr::Timeout) => "실패: 상대 연결 시간초과 — 상대 PC 가 켜져 있고 방화벽 허용됐는지 확인.".into(),
        Err(SendErr::Other(e)) => format!("실패: {}", e),
    }
}
