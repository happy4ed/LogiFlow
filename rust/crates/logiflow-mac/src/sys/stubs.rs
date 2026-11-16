//! Mac 에 없는 묶음 — Windows 전용 기능의 빈 구현. 설정 창은 `dialog::CAPS` 로 이 줄들을 숨기므로 보통 불리지 않는다.

/// Windows 방화벽(Mac 은 들어오는 연결을 시스템이 처음 한 번 묻는다).
pub mod firewall {
    pub fn allowed_for_port(_port: i32) -> bool {
        true
    }
    pub fn request_allow(_port: i32) -> bool {
        true
    }
    pub fn request_remove() -> bool {
        true
    }
}

/// 기존 LogiFlow(C# 판, Windows) 정리 — Mac 에는 없다. Swift 판은 따로 지울 필요가 없다(나란히 설치).
pub mod legacy {
    use logiflow_core::log::DiagLogger;

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Leftovers {
        pub installed: bool,
        pub tray_running: bool,
        pub service: bool,
        pub machine_dir: bool,
        pub run_value: bool,
    }

    impl Leftovers {
        pub fn any(&self) -> bool {
            self.installed || self.tray_running || self.service || self.machine_dir || self.run_value
        }
        pub fn conflicts(&self) -> bool {
            self.tray_running || self.service
        }
        pub fn describe(&self) -> String {
            String::new()
        }
    }

    pub fn leftovers() -> Leftovers {
        Leftovers::default()
    }
    pub fn conflict() -> bool {
        false
    }
    pub fn cleanup_user(_log: &DiagLogger) -> Vec<String> {
        Vec::new()
    }
    pub fn request_cleanup_machine_only() -> (bool, Vec<String>) {
        (true, Vec::new())
    }
}

/// 로그인 전에도 실행(Windows 서비스) — Mac 에는 없다.
pub mod prelogon {
    pub fn is_installed() -> bool {
        false
    }
    pub fn request_enable(_my_host: i32) -> bool {
        false
    }
    pub fn request_disable() -> bool {
        true
    }
}

/// 로그인 화면 테스트(Windows) — Mac 에는 없다.
pub mod probe {
    pub fn is_installed() -> bool {
        false
    }
    pub fn request_install_run() -> bool {
        false
    }
    pub fn request_uninstall() -> bool {
        true
    }
}
