//! 로그인 시 자동 시작 — `~/Library/LaunchAgents/com.happy4ed.logiflow.rs.plist`(RunAtLoad). 관리자 불필요.
//! 다음 로그인부터 적용된다(지금 도는 상주 앱과 겹치지 않게 launchctl 로 바로 올리지 않는다).
//! 실행 파일 자리가 바뀌어도(앱을 옮김) 상주 앱이 뜰 때마다 지금 자리로 다시 쓴다.

use super::paths;
use super::util::exe_path;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn plist_text(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>--autostart</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><false/>
  <key>ProcessType</key><string>Interactive</string>
  <key>LimitLoadToSessionType</key><string>Aqua</string>
</dict>
</plist>
"#,
        label = paths::LAUNCH_AGENT_LABEL,
        exe = xml_escape(exe)
    )
}

pub fn apply(enable: bool) {
    let path = paths::launch_agent_path();
    if enable {
        let text = plist_text(&exe_path().to_string_lossy());
        if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
            return;
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    } else {
        let _ = std::fs::remove_file(&path);
    }
}

pub fn is_enabled() -> bool {
    paths::launch_agent_path().exists()
}
