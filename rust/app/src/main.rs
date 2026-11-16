//! LogiFlowRs.exe — C# `Program.Main` 과 같은 인자 분기. 인자 없으면 트레이.
//! 러스트 판에서 더한 인자: `--ui`(설정 창 프로세스) · `--uninstall-cleanup` / `--uninstall-machine`(제거 정리).
//! Mac(`LogiFlowRs.app/Contents/MacOS/LogiFlowRs`): 인자 없으면 메뉴 막대 상주 + 설정 창, `--autostart`(로그인 항목)는
//! 상주만, `--ui` 설정 창, `--ui-snapshot` CI 캡처, `--version`.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    use logiflow_win::{firewall, prelogon, probe, service, tray, workers};
    let version = env!("CARGO_PKG_VERSION");
    let arg = std::env::args().nth(1).unwrap_or_default();
    let (name, value) = match arg.split_once('=') {
        Some((n, v)) => (n.to_ascii_lowercase(), Some(v.to_string())),
        None => (arg.to_ascii_lowercase(), None),
    };
    let num = |def: i32| value.as_deref().and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(def);
    let code = match name.as_str() {
        // 윈도우 서비스로 실행(SCM 이 LocalSystem·세션0 에서 호출).
        "--service" => {
            service::run();
            0
        }
        "--prelogon-pull" => {
            prelogon::run_pull(num(-1));
            0
        }
        "--install-prelogon" => prelogon::do_install(num(-1)),
        "--uninstall-prelogon" => prelogon::do_uninstall(),
        // 로그온 화면 접근 '가능성 확인' 프로브(읽기 전용 · setHost 없음).
        "--desktop-probe-launch" => {
            probe::run_launcher("--desktop-probe=100");
            0
        }
        "--desktop-probe" => {
            probe::run_probe(num(120));
            0
        }
        // 로그온/잠금 화면 실제 전환: launch(세션0) → winlogon 데스크톱에 worker 투입.
        "--logon-switch-launch" => {
            probe::run_launcher("--logon-switch");
            0
        }
        // B안: 바탕화면(Default) raw 엣지 전환 워커. 서비스가 winsta0\default 에 스폰.
        "--desktop-switch" => {
            workers::run_desktop_switch();
            0
        }
        "--logon-switch" => {
            workers::run_logon_switch();
            0
        }
        "--install-probe" => probe::do_install_run(),
        "--uninstall-probe" => probe::do_uninstall(),
        // 클립보드 수신 포트 방화벽 허용(상승된 인스턴스에서 netsh).
        "--allow-firewall" => firewall::do_allow(num(logiflow_core::clip::DEFAULT_PORT)),
        // 러스트 판: 설정 창 「허용 해제」(상승된 인스턴스).
        "--remove-firewall" => firewall::do_remove(),
        // 러스트 판: 설정 창 별도 프로세스.
        "--ui" => {
            logiflow_ui::run(version);
            0
        }
        // CI 화면 대조 캡처(트레이 없이 시험 자료).
        "--ui-snapshot" => {
            logiflow_ui::run_snapshot(version, value.as_deref().unwrap_or("snapshots"));
            0
        }
        // 러스트 판: 제거 정리(지금 판과 다른 점 — 서비스·워커 사본까지 지움).
        "--uninstall-cleanup" => prelogon::uninstall_cleanup(),
        // 기존 LogiFlow(C# 판) 정리 — 사용자 쪽 + 머신 쪽(관리자로 실행할 때·CI), 머신 쪽만(설정 창이 UAC 로 부름).
        "--cleanup-legacy" => {
            let log = logiflow_core::log::DiagLogger::new(&logiflow_win::util::exe_dir(), true);
            logiflow_win::legacy::cleanup_user(&log);
            logiflow_win::legacy::cleanup_machine()
        }
        "--cleanup-legacy-machine" => logiflow_win::legacy::cleanup_machine(),
        "--uninstall-machine" => prelogon::uninstall_machine(),
        _ => {
            tray::run();
            0
        }
    };
    std::process::exit(code);
}

#[cfg(target_os = "macos")]
fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let arg = std::env::args().nth(1).unwrap_or_default();
    let (name, value) = match arg.split_once('=') {
        Some((n, v)) => (n.to_ascii_lowercase(), Some(v.to_string())),
        None => (arg.to_ascii_lowercase(), None),
    };
    match name.as_str() {
        "--version" | "-v" => println!("LogiFlow {}", version),
        "--ui" => logiflow_ui::run(version),
        "--ui-snapshot" => logiflow_ui::run_snapshot(version, value.as_deref().unwrap_or("snapshots")),
        // 로그인 항목(LaunchAgent)으로 뜰 때 — 창 없이 메뉴 막대만.
        "--autostart" => logiflow_mac::tray::run(false),
        // 사람이 앱을 열었을 때(Finder 의 -psn_ 인자 등 모르는 인자 포함) — 메뉴 막대 + 설정 창.
        _ => logiflow_mac::tray::run(true),
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn main() {
    eprintln!("LogiFlowRs 는 Windows · Mac 판입니다. Linux 는 logiflow-rs(CLI)를 쓰세요.");
}
