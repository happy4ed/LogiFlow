//! 코어 자동 시험 — 계획서 4.1~4.4 · 4.6~4.10 자동분.

use crate::cache;
use crate::clip;
use crate::config::{AppConfig, EdgeAttachment, SecretCodec, ThemeMode};
use crate::edge::*;
use crate::glow::{self, EdgeGlow, GlowAction, GlowState};
use crate::hidpp::{build_report, HidTransport};
use crate::log::DiagLogger;
use crate::logon_edges;
use crate::scan::{self, HidBackend, HidInterfaceInfo, ScanResult};
use crate::switcher;
use crate::theme::{Rgb, LIGHT};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// ---------- 가짜 로지텍 기기 ----------

#[derive(Clone)]
struct FakeDev {
    index: u8,
    name: &'static str,
    ch_index: u8, // 0 = 0x1814 미지원
    name_index: u8,
    hosts: u8,
    current: u8,
    /// setHost 를 받으면 사라짐(응답 없음).
    gone: bool,
}

#[derive(Clone)]
struct FakeIface {
    info: HidInterfaceInfo,
    devs: Vec<FakeDev>,
}

#[derive(Default)]
struct Shared {
    writes: Vec<(String, Vec<u8>)>,
}

struct FakeBackend {
    ifaces: Arc<Mutex<Vec<FakeIface>>>,
    shared: Arc<Mutex<Shared>>,
}

struct FakeStream {
    path: String,
    out_len: usize,
    ifaces: Arc<Mutex<Vec<FakeIface>>>,
    shared: Arc<Mutex<Shared>>,
    queue: VecDeque<Vec<u8>>,
}

impl HidTransport for FakeStream {
    fn output_report_len(&self) -> usize {
        self.out_len
    }
    fn write(&mut self, report: &[u8], _t: u32, _token: crate::hidpp::WriteToken) -> bool {
        self.shared.lock().unwrap().writes.push((self.path.clone(), report.to_vec()));
        let (rid, dev, feat, fb) = (report[0], report[1], report[2], report[3]);
        let func = fb >> 4;
        let mut ifs = self.ifaces.lock().unwrap();
        let iface = ifs.iter_mut().find(|i| i.info.path == self.path).unwrap();
        let d = match iface.devs.iter_mut().find(|d| d.index == dev) {
            Some(d) => d,
            None => return true, // 아무도 응답 안 함
        };
        if d.gone {
            return true;
        }
        let mut resp = vec![0u8; if rid == 0x11 { 20 } else { 7 }];
        resp[0] = rid;
        resp[1] = dev;
        resp[2] = feat;
        resp[3] = fb;
        match (feat, func) {
            (0, 1) => resp[6] = report[6], // ping echo
            (0, 0) => {
                let fid = ((report[4] as u16) << 8) | report[5] as u16;
                resp[4] = match fid {
                    0x1814 => d.ch_index,
                    0x0005 => d.name_index,
                    _ => 0,
                };
            }
            (f, 0) if f == d.ch_index && f != 0 => {
                resp[4] = d.hosts;
                resp[5] = d.current;
            }
            (f, 1) if f == d.ch_index && f != 0 => {
                d.current = report[4];
                d.gone = true;
                return true;
            }
            (f, 0) if f == d.name_index && f != 0 => resp[4] = d.name.len() as u8,
            (f, 1) if f == d.name_index && f != 0 => {
                let start = report[4] as usize;
                let max = resp.len() - 4;
                for (k, b) in d.name.bytes().skip(start).take(max).enumerate() {
                    resp[4 + k] = b;
                }
            }
            _ => {
                // HID++2.0 오류 응답(InvalidFunctionId)
                resp[2] = 0xFF;
                resp[3] = feat;
                resp[4] = fb;
                resp[5] = 7;
            }
        }
        drop(ifs);
        // 비동기 알림 하나를 먼저 끼워 넣어 "알림은 건너뜀"을 같이 확인.
        let mut noise = vec![0u8; resp.len()];
        noise[0] = rid;
        noise[1] = dev;
        noise[2] = 0x04;
        noise[3] = 0x00;
        self.queue.push_back(noise);
        self.queue.push_back(resp);
        true
    }
    fn read(&mut self, _t: u32) -> Option<Vec<u8>> {
        self.queue.pop_front()
    }
}

impl HidBackend for FakeBackend {
    fn enumerate(&self, _v: u16, _p: u16) -> Vec<HidInterfaceInfo> {
        self.ifaces.lock().unwrap().iter().map(|i| i.info.clone()).collect()
    }
    fn open(&self, info: &HidInterfaceInfo) -> Result<Box<dyn HidTransport>, String> {
        Ok(Box::new(FakeStream {
            path: info.path.clone(),
            out_len: info.output_report_len,
            ifaces: self.ifaces.clone(),
            shared: self.shared.clone(),
            queue: VecDeque::new(),
        }))
    }
}

fn iface(path: &str, usage: u16, out: usize) -> HidInterfaceInfo {
    HidInterfaceInfo {
        path: path.into(),
        vendor_id: 0x046D,
        product_id: 0xC548,
        usage_page: 0xFF00,
        usage,
        input_report_len: out,
        output_report_len: out,
        product_string: Some("USB Receiver".into()),
        caps_read: true,
        device_index_hint: None,
    }
}

fn mx() -> FakeDev {
    FakeDev {
        index: 1,
        name: "MX Master 4",
        ch_index: 0x0A,
        name_index: 0x03,
        hosts: 3,
        current: 0,
        gone: false,
    }
}
fn keys() -> FakeDev {
    FakeDev {
        index: 2,
        name: "MX Keys",
        ch_index: 0x09,
        name_index: 0x03,
        hosts: 3,
        current: 0,
        gone: false,
    }
}
fn old_mouse() -> FakeDev {
    FakeDev {
        index: 3,
        name: "M100",
        ch_index: 0,
        name_index: 0,
        hosts: 0,
        current: 0,
        gone: false,
    }
}

fn backend(devs: Vec<FakeDev>) -> FakeBackend {
    let ifaces = vec![
        FakeIface {
            info: iface(r"\\?\hid#vid_046d&pid_c548&mi_02&col01", 0x0201, 7),
            devs: devs.clone(),
        },
        FakeIface {
            info: iface(r"\\?\hid#vid_046d&pid_c548&mi_02&col02", 0x0202, 20),
            devs,
        },
        FakeIface {
            info: {
                let mut i = iface(r"\\?\hid#vid_046d&pid_c548&mi_00", 0x0002, 0);
                i.usage_page = 1;
                i
            },
            devs: vec![],
        },
    ];
    FakeBackend {
        ifaces: Arc::new(Mutex::new(ifaces)),
        shared: Arc::new(Mutex::new(Shared::default())),
    }
}

/// 쓰기 분류: setHost(0x1814 func1)이면 true.
fn is_set_host(report: &[u8], ch_indices: &[u8]) -> bool {
    report[2] != 0 && ch_indices.contains(&report[2]) && (report[3] >> 4) == 1
}

// ---------- 4.1 프레임 ----------

#[test]
fn frame_long_and_short() {
    assert_eq!(build_report(20, 0xFF, 0x00, 1, &[0, 0, 0x5A]), {
        let mut v = vec![0u8; 20];
        v[0] = 0x11;
        v[1] = 0xFF;
        v[3] = 0x1A;
        v[6] = 0x5A;
        v
    });
    assert_eq!(build_report(7, 0x01, 0x0A, 1, &[2]), vec![0x10, 0x01, 0x0A, 0x1A, 0x02, 0, 0]);
    // short 는 파라미터 3바이트까지만.
    assert_eq!(build_report(7, 1, 2, 0, &[1, 2, 3, 4, 5]), vec![0x10, 1, 2, 0x0A, 1, 2, 3]);
}

// ---------- 4.2 · 4.3 스캔 ----------

#[test]
fn scan_finds_devices_and_dedupes() {
    let be = backend(vec![mx(), keys(), old_mouse()]);
    let log = DiagLogger::null();
    let r = scan::scan(&be, &log);
    // long/short 두 인터페이스에 같은 기기 3개 → 중복 제거 후 3개.
    assert_eq!(r.len(), 3, "{:?}", r.iter().map(|x| &x.device_name).collect::<Vec<_>>());
    let m = r.iter().find(|x| x.device_name == "MX Master 4").unwrap();
    assert!(m.change_host_supported);
    assert_eq!(m.change_host_index, 0x0A);
    assert_eq!((m.host_count, m.current_host), (3, 0));
    assert_eq!(m.connection_type, "리시버 뒤(index 1)");
    let o = r.iter().find(|x| x.device_index == 3).unwrap();
    assert!(!o.change_host_supported);
    assert_eq!(o.note, "0x1814 미지원");
    // 이름 기능 없음 → 제품 문자열.
    assert_eq!(o.device_name, "USB Receiver");
    assert_eq!(scan::switchable(&r).len(), 2);
}

// ---------- 4.4 안전 규칙 ----------

#[test]
fn scan_never_writes_set_host() {
    let be = backend(vec![mx(), keys(), old_mouse()]);
    scan::scan(&be, &DiagLogger::null());
    let w = &be.shared.lock().unwrap().writes;
    assert!(!w.is_empty());
    for (_, rep) in w {
        assert!(!is_set_host(rep, &[0x0A, 0x09]), "스캔 중 setHost: {:02X?}", rep);
        // 조회는 func0/func1 만, 피처는 IRoot·0x1814·0x0005 인덱스뿐.
        assert!([0x00u8, 0x0A, 0x09, 0x03].contains(&rep[2]), "알 수 없는 피처: {:02X?}", rep);
        assert!(rep[3] >> 4 <= 1);
    }
}

#[test]
fn switch_guards_send_nothing_when_violated() {
    let be = backend(vec![mx(), old_mouse()]);
    let log = DiagLogger::null();
    let r = scan::scan(&be, &log);
    be.shared.lock().unwrap().writes.clear();
    let m = r.iter().find(|x| x.device_name == "MX Master 4").unwrap();
    let o = r.iter().find(|x| x.device_index == 3).unwrap();
    // (a) 미지원 기기
    assert!(!switcher::switch_one(&be, o, 1, &log));
    // (b) 범위 밖
    assert!(!switcher::switch_one(&be, m, 3, &log));
    assert!(!switcher::switch_one(&be, m, -1, &log));
    assert!(be.shared.lock().unwrap().writes.is_empty());
    // 정상: setHost 정확히 1건.
    assert!(switcher::switch_one(&be, m, 1, &log));
    let w = be.shared.lock().unwrap().writes.clone();
    assert_eq!(w.len(), 1);
    assert!(is_set_host(&w[0].1, &[0x0A]));
    assert_eq!(w[0].1[1], 1);
    assert_eq!(w[0].1[4], 1);
}

#[test]
fn burst_switches_all_and_reports_delivery() {
    let be = backend(vec![mx(), keys()]);
    let log = DiagLogger::null();
    let r = scan::switchable(&scan::scan(&be, &log));
    be.shared.lock().unwrap().writes.clear();
    let delivered = std::cell::Cell::new(0);
    let mut lines = Vec::new();
    let ok = switcher::switch_all_burst(
        &be,
        &r,
        2,
        &log,
        &mut |s| lines.push(s.to_string()),
        3,
        Some(&|| delivered.set(delivered.get() + 1)),
    );
    assert_eq!(ok, 2);
    assert_eq!(delivered.get(), 1);
    // 쓰기 = setHost 2건 + 확인 조회(getHostInfo) 2건. 그 밖 쓰기 없음.
    let w = be.shared.lock().unwrap().writes.clone();
    let sets: Vec<_> = w.iter().filter(|(_, x)| is_set_host(x, &[0x0A, 0x09])).collect();
    assert_eq!(sets.len(), 2);
    assert_eq!(w.len(), 4);
    assert!(lines.iter().all(|l| l.contains("응답없음(전환추정)") && l.contains("전환됨")), "{:?}", lines);
}

#[test]
fn verbose_switch_retries_and_confirms() {
    let be = backend(vec![mx()]);
    let log = DiagLogger::null();
    let r = scan::switchable(&scan::scan(&be, &log));
    let mut lines = Vec::new();
    let ok = switcher::switch_all_verbose(&be, &r, 1, &log, &mut |s| lines.push(s.to_string()), 3);
    assert_eq!(ok, 1);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("  MX Master 4: 시도1 쓰기=성공"), "{}", lines[0]);
}

// ---------- 4.6 가장자리 ----------

#[test]
fn edge_requires_dwell_and_push() {
    let mon = HandoffRect::new(0, 0, 1920, 1080);
    let mut d = EdgeDetector::default();
    let x = 1919;
    assert!(!d.feed(&mon, HandoffEdge::Right, x, 500, 5, 0, false, false)); // 시작
    assert!(!d.feed(&mon, HandoffEdge::Right, x, 500, 5, 100, false, false));
    assert!(!d.feed(&mon, HandoffEdge::Right, x, 500, 0, 340, false, false)); // dwell 됨, 밀기 5 < 10
    assert!(d.feed(&mon, HandoffEdge::Right, x, 500, 6, 360, false, false)); // 밀기 11
    assert!(!d.armed());
    // 재무장 전엔 다시 안 됨.
    assert!(!d.feed(&mon, HandoffEdge::Right, x, 500, 50, 2000, false, false));
    assert!(!d.feed(&mon, HandoffEdge::Right, 1919 - 16, 500, 0, 2010, false, false));
    assert!(d.armed());
}

#[test]
fn edge_button_down_and_band() {
    let mon = HandoffRect::new(0, 0, 1920, 1080);
    let mut d = EdgeDetector::default();
    d.feed(&mon, HandoffEdge::Left, 0, 10, 0, 0, false, false);
    // 버튼 누름 → 리셋.
    assert!(!d.feed(&mon, HandoffEdge::Left, 0, 10, 30, 400, false, true));
    assert_eq!(d.progress(400), 0.0);
    // 띠(2px) 밖 → 리셋.
    d.feed(&mon, HandoffEdge::Left, 0, 10, 0, 500, false, false);
    assert!(!d.feed(&mon, HandoffEdge::Left, 3, 10, 30, 900, false, false));
    // 주입 입력은 무시.
    assert!(!d.feed(&mon, HandoffEdge::Left, 0, 10, 30, 900, true, false));
    // 밀기 0px 요건(로그온 워커): dwell 만으로.
    let mut z = EdgeDetector::new(330, 0);
    z.feed(&mon, HandoffEdge::Top, 5, 0, 0, 0, false, false);
    assert!(z.feed(&mon, HandoffEdge::Top, 5, 0, 0, 330, false, false));
    assert_eq!(EdgeDetector::outward_push(HandoffEdge::Left, -4, 0), 4);
    assert_eq!(EdgeDetector::outward_push(HandoffEdge::Bottom, 0, -4), 0);
}

#[test]
fn layout_snap_and_resolve() {
    let mons = vec![HandoffRect::new(14, 14, 180, 101)];
    let s = snap_flush(&mons[0], HandoffEdge::Right, 180, 101);
    assert_eq!((s.left, s.top), (194, 14));
    let near = HandoffRect::new(200, 20, 180, 101);
    assert_eq!(resolve_attachment(&mons, &near, 30), Some((0, HandoffEdge::Right)));
    let far = HandoffRect::new(260, 20, 180, 101);
    assert_eq!(resolve_attachment(&mons, &far, 30), None);
    let below = HandoffRect::new(20, 120, 180, 101);
    assert_eq!(resolve_attachment(&mons, &below, 30), Some((0, HandoffEdge::Bottom)));
}

// ---------- 4.7 트레이 감시 켜는 조건 ----------

#[test]
fn tray_watch_needs_all_four_conditions() {
    let mut c = AppConfig::default();
    c.pre_logon = false;
    c.edge_switch_enabled = true;
    assert!(c.tray_should_watch(true, 1));
    assert!(!c.tray_should_watch(false, 1), "배치 없음");
    assert!(!c.tray_should_watch(true, 0), "전환 가능 기기 없음");
    c.edge_switch_enabled = false;
    assert!(!c.tray_should_watch(true, 1), "가장자리 자동 전환 꺼짐");
    c.edge_switch_enabled = true;
    c.pre_logon = true;
    assert!(!c.tray_should_watch(true, 1), "서비스(로그온 전에도 실행) 켜짐 → 워커가 전담");
}

// ---------- rs.3 · 화면 이름 · 가드 · 테스트 판정 ----------

#[test]
fn channel_label_does_not_repeat_default_name() {
    let mut c = AppConfig::default();
    c.set_alias(1, "노트북");
    assert_eq!(c.channel_label(1), "채널 2 · 노트북");
    assert_eq!(c.channel_label(2), "채널 3");
    c.set_alias(1, "");
    assert_eq!(c.channel_label(1), "채널 2");
}

#[test]
fn unknown_host_count_sends_nothing() {
    let be = backend(vec![mx()]);
    let log = DiagLogger::null();
    let mut r = scan::scan(&be, &log);
    be.shared.lock().unwrap().writes.clear();
    let m = r.iter_mut().find(|x| x.device_name == "MX Master 4").unwrap();
    m.host_count = -1; // getHostInfo 무응답
    assert!(!switcher::switch_one(&be, m, 0, &log));
    assert!(!switcher::switch_one(&be, m, 1, &log));
    let one = vec![m.clone()];
    assert_eq!(switcher::switch_all(&be, &one, 1, &log), 0);
    assert!(be.shared.lock().unwrap().writes.is_empty());
}

#[test]
fn probe_result_reads_last_run() {
    use crate::probe_result::{verdict, ProbeVerdict};
    let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(verdict(&l(&["서비스 시작"])), ProbeVerdict::NotRun);
    let base = "11:19:34.596  desktop-probe 시작 · 실행 계정=SYSTEM · 120초";
    let def = "11:19:35.801  desktop=\"Default\" setThreadDesktop=성공(err0) GetCursorPos=(512,384)(err0)";
    let end = "11:20:00.000  desktop-probe 종료. 위에서 desktop=\"Winlogon\" 줄의 GetCursorPos 가 '성공(좌표 변함)'이면 …";
    assert_eq!(verdict(&l(&[base, def, end])), ProbeVerdict::NoSecureScreen);
    let w1 = "11:19:40.000  desktop=\"Winlogon\" setThreadDesktop=성공(err0) GetCursorPos=(10,20)(err0)";
    let w2 = "11:19:41.800  desktop=\"Winlogon\" setThreadDesktop=성공(err0) GetCursorPos=(300,20)(err0)";
    let wf = "11:19:40.000  desktop=\"Winlogon\" setThreadDesktop=실패(err5) GetCursorPos=실패(err0)";
    assert_eq!(verdict(&l(&[base, def, w1, w2, end])), ProbeVerdict::Works);
    assert_eq!(verdict(&l(&[base, w1, w1, end])), ProbeVerdict::NoMovement);
    assert_eq!(verdict(&l(&[base, wf, end])), ProbeVerdict::Blocked);
    assert_eq!(
        verdict(&l(&[base, "11:19:40.000  OpenInputDesktop 실패 (입력 데스크톱 접근 거부) err=5", end])),
        ProbeVerdict::Blocked
    );
    // 앞 실행이 성공이어도 마지막 실행 기준.
    assert_eq!(verdict(&l(&[base, w1, w2, end, base, def, end])), ProbeVerdict::NoSecureScreen);
}

// ---------- 4.9 파일 형식 ----------

const CSHARP_CONFIG: &str = "\u{feff}# LogiFlow 설정\r\nedgeEnabled=1\r\ndwellMs=500\r\nautostart=1\r\ndragGuard=0\r\ndiagLog=1\r\npreLogon=1\r\nmyHost=0\r\nfontOffset=3\r\nclipEnabled=1\r\nclipHost=192.168.0.7\r\nclipPort=48737\r\nclipSecret=비밀123\r\ndebug=0\r\ntheme=2\r\nalias=0\t회사 PC\r\nalias=1\t노트북\r\nattach=1\t\\\\.\\DISPLAY2\t1\r\n";

#[test]
fn config_reads_csharp_file() {
    let dir = std::env::temp_dir().join(format!("lf-cfg-{}", std::process::id()));
    let p = dir.join("config.txt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&p, CSHARP_CONFIG).unwrap();
    let c = AppConfig::load(&p, None);
    assert_eq!(c.dwell_ms, 500);
    assert!(!c.drag_guard);
    assert!(c.pre_logon);
    assert_eq!(c.font_offset, 3);
    assert_eq!(c.clip_secret, "비밀123");
    assert_eq!(c.theme, ThemeMode::Dark);
    assert_eq!(c.alias_for(0), "회사 PC");
    assert_eq!(c.alias_for(2), "채널 3");
    assert_eq!(
        c.attachments,
        vec![EdgeAttachment {
            channel: 1,
            monitor_device_name: r"\\.\DISPLAY2".into(),
            edge: HandoffEdge::Right
        }]
    );
    // 평문 왕복.
    assert_eq!(AppConfig::from_plain_text(&c.serialize(None)), c);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_secret_protected_on_save() {
    let mut c = AppConfig {
        clip_secret: "pw".into(),
        ..Default::default()
    };
    let protect = |s: &str| Some(format!("P{}", s.bytes().map(|b| format!("{:02x}", b)).collect::<String>()));
    let unprotect = |h: &str| {
        let h = h.strip_prefix('P')?;
        let bytes: Vec<u8> = (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect();
        String::from_utf8(bytes).ok()
    };
    let codec = SecretCodec {
        protect: &protect,
        unprotect: &unprotect,
    };
    let text = c.serialize(Some(&codec));
    assert!(text.contains("clipSecret=\r\n") || text.contains("clipSecret=\n"));
    assert!(!text.contains("clipSecret=pw"));
    assert!(text.contains("clipSecretDpapi=P7077"));
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    assert_eq!(AppConfig::parse(&lines, Some(&codec)).clip_secret, "pw");
    // 보호 실패 → 암호문구는 아예 안 씀.
    let fail = |_: &str| None;
    let codec2 = SecretCodec {
        protect: &fail,
        unprotect: &unprotect,
    };
    c.clip_secret = "x".into();
    assert!(!c.serialize(Some(&codec2)).contains("clipSecretDpapi"));
}

#[test]
fn cache_roundtrip() {
    let r = ScanResult {
        interface: Some(HidInterfaceInfo {
            path: r"\\?\hid#vid_046d&pid_c548".into(),
            output_report_len: 20,
            input_report_len: 20,
            usage_page: 0xFF00,
            caps_read: true,
            ..Default::default()
        }),
        device_name: "MX\tMaster".into(),
        vid_pid: "046D:C548".into(),
        path: r"\\?\hid#vid_046d&pid_c548".into(),
        connection_type: "리시버 뒤(index 1)".into(),
        device_index: 1,
        change_host_supported: true,
        change_host_index: 10,
        host_count: 3,
        current_host: 1,
        note: "".into(),
    };
    let text = cache::serialize(std::slice::from_ref(&r));
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    assert_eq!(cache::parse(&lines), vec![r]);
}

#[test]
fn logon_edges_roundtrip() {
    let mut c = AppConfig::default();
    c.attachments.push(EdgeAttachment {
        channel: 2,
        monitor_device_name: r"\\.\DISPLAY2".into(),
        edge: HandoffEdge::Top,
    });
    c.drag_guard = false;
    let names = vec![r"\\.\DISPLAY1".to_string(), r"\\.\DISPLAY2".to_string()];
    let text = logon_edges::serialize(&c, &names);
    assert!(text.contains("edge\t\\\\.\\DISPLAY2\t1\t2\t2"));
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let f = logon_edges::parse(&lines);
    assert_eq!(f.dwell_ms, 330);
    assert!(!f.drag_guard);
    assert_eq!(f.edges[0].index, 1);
    // C# 판 파일(dragGuard 줄 없음) → 기본 켜짐.
    let old = logon_edges::parse(&["dwellMs=500".to_string()]);
    assert!(old.drag_guard);
    assert_eq!(old.dwell_ms, 500);
}

// ---------- 4.10 클립보드 ----------

#[test]
fn clip_matches_independent_vector() {
    // 시험 벡터: Python hashlib.pbkdf2_hmac + cryptography AES-CBC + hmac(독립 구현)으로 생성.
    assert_eq!(clip::norm("  비밀１２３ "), "비밀123");
    let k = clip::derive_keys("  비밀１２３ ");
    let hex = |b: &[u8]| b.iter().map(|x| format!("{:02x}", x)).collect::<String>();
    assert_eq!(hex(&k.aes), "97a903290fc2f3516137c3579fe3aab2c1b0ea4881ee1d74cf328e9ccfcb0f03");
    assert_eq!(hex(&k.mac), "6cff003a638e4677e64008748b82229fd77de5aaf7c7fc4fbe297aa87c0aa317");
    let iv: [u8; 16] = core::array::from_fn(|i| i as u8);
    let frame = clip::encrypt_with_iv(&k, &clip::text_payload("안녕 LogiFlow"), &iv);
    assert_eq!(
        hex(&frame),
        "000102030405060708090a0b0c0d0e0fbc846dc1ac4a60aea7d0599acaa95a646dacf83cc06ac1707d77d2b0ee48baf23e6b4560e6357878290d51a0026fe648b7222aff24bb21628e40689e114b5ae2"
    );
    assert_eq!(clip::decrypt(&k, &frame).unwrap(), clip::text_payload("안녕 LogiFlow"));
    // 변조·다른 암호문구 → 거부.
    let mut bad = frame.clone();
    bad[20] ^= 1;
    assert!(clip::decrypt(&k, &bad).is_none());
    assert!(clip::decrypt(&clip::derive_keys("other"), &frame).is_none());
    assert_eq!(&clip::wire(&frame)[..4], &(frame.len() as i32).to_le_bytes());
    assert!(!clip::valid_len(0) && clip::valid_len(100) && !clip::valid_len(8 * 1024 * 1024 + 4097));
}

// ---------- 4.8 glow ----------

#[test]
fn glow_states_follow_rules() {
    let mon = HandoffRect::new(0, 0, 1920, 1080);
    let mut g = GlowState::default();
    g.update(
        &EdgeGlow {
            active: true,
            monitor: mon,
            edge: Some(HandoffEdge::Right),
            progress: 1.0,
            ..Default::default()
        },
        0,
    );
    let mut last = 0.0;
    for t in 0..30 {
        if let GlowAction::Render { progress, peak } = g.tick(t * 16) {
            assert!(!peak);
            assert!(progress <= glow::DWELL_CEILING + 1e-9);
            last = progress;
        }
    }
    assert!((last - 0.8).abs() < 0.01, "dwell 상한 80%: {}", last);
    // 전환 요청 → 90% 대기 → 전달 순간 최고점 → 3틱 뒤 숨김.
    g.update(
        &EdgeGlow {
            fired: true,
            active: true,
            monitor: mon,
            edge: Some(HandoffEdge::Right),
            progress: 1.0,
            ..Default::default()
        },
        500,
    );
    // 전환 중엔 다른 입력 무시.
    g.update(
        &EdgeGlow {
            active: false,
            ..Default::default()
        },
        510,
    );
    for t in 0..20 {
        g.tick(520 + t * 16);
    }
    assert_eq!(g.delivered(), Some(GlowAction::Render { progress: 1.0, peak: true }));
    assert_eq!(g.tick(900), GlowAction::None);
    assert_eq!(g.tick(916), GlowAction::None);
    assert_eq!(g.tick(932), GlowAction::HideAndStop);
    // 전달 신호 없이 1.5초 → 흐려져 숨김.
    let mut h = GlowState::default();
    h.update(
        &EdgeGlow {
            fired: true,
            active: true,
            monitor: mon,
            edge: Some(HandoffEdge::Left),
            progress: 1.0,
            ..Default::default()
        },
        0,
    );
    let mut hid = false;
    for t in 0..400 {
        if h.tick(t * 16) == GlowAction::HideAndStop {
            hid = true;
            assert!(t * 16 > glow::SEND_TIMEOUT_MS);
            break;
        }
    }
    assert!(hid);
}

#[test]
fn glow_bitmap_shape() {
    let mon = HandoffRect::new(-1920, 0, 1920, 1080);
    let acc = LIGHT.accent;
    let r = glow::render(&mon, HandoffEdge::Right, 0.5, false, acc).unwrap();
    assert_eq!((r.width, r.height, r.x, r.y), (90, 1080, -90, 0));
    // 변(오른쪽 끝)이 가장 진하고 안쪽 끝은 거의 투명.
    let a = |p: u32| p >> 24;
    assert!(a(r.pixels[89]) > 200);
    assert!(a(r.pixels[0]) < 5);
    let b = glow::render(&mon, HandoffEdge::Bottom, 0.5, false, acc).unwrap();
    assert_eq!((b.width, b.height, b.y), (1920, 90, 990));
    assert!(a(b.pixels[(89 * 1920) as usize]) > 200);
    let t = glow::render(&mon, HandoffEdge::Top, 0.5, false, acc).unwrap();
    assert!(a(t.pixels[0]) > 200);
    let l = glow::render(&mon, HandoffEdge::Left, 0.5, false, acc).unwrap();
    assert_eq!(l.x, -1920);
    assert!(a(l.pixels[0]) > 200);
    // 진행 0 → 완전 투명.
    let z = glow::canonical_row(0.0, false, acc);
    assert!(z.iter().all(|p| *p == 0));
    // 최고점은 흰빛.
    let peak = glow::canonical_row(1.0, true, acc);
    assert_eq!(peak[89], 0xFFFF_FFFF);
    assert_eq!(crate::theme::blend(Rgb(255, 255, 255), Rgb(0, 0x7A, 0xFF), 0.5), Rgb(127, 188, 255));
}
