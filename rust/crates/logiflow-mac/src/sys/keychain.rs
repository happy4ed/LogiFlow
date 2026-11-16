//! 클립보드 암호문구 보관 — macOS 키체인(로그인 키체인의 일반 암호 항목, Security framework).
//! Windows 의 DPAPI 자리. 설정 파일(`config.txt`)에는 암호문구 대신 `clipSecretDpapi=keychain` 표시만 남는다.
//!
//! 실패하면 평문으로 대신 쓰지 않는다: 저장 실패 → 파일에는 암호문구를 쓰지 않고(메모리에는 있어 지금은 주고받음)
//! 설정 창 암호문구 줄에 주의 글. 읽기 실패 → 암호문구 없음으로 시작하고 같은 줄에 「다시 넣어 주세요」.
//! 키체인 항목: 서비스 `com.happy4ed.logiflow.rs` · 계정 `clipSecret`.

use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};
use std::sync::Mutex;

pub const SERVICE: &str = "com.happy4ed.logiflow.rs";
pub const ACCOUNT: &str = "clipSecret";
/// 설정 파일에 남기는 표시(코어 `clipSecretDpapi=` 자리).
pub const MARK: &str = "keychain";
/// errSecItemNotFound
const NOT_FOUND: i32 = -25300;

/// 지금 문제(설정 창 주의 글). None = 문제 없음.
static NOTE: Mutex<Option<String>> = Mutex::new(None);

pub fn note() -> Option<String> {
    NOTE.lock().ok().and_then(|n| n.clone())
}

fn set_note(n: Option<String>) {
    if let Ok(mut g) = NOTE.lock() {
        *g = n;
    }
}

/// 저장하고 다시 읽어 같은지까지 확인.
pub fn store(plain: &str) -> Result<(), String> {
    if let Ok(cur) = get_generic_password(SERVICE, ACCOUNT) {
        if cur == plain.as_bytes() {
            set_note(None);
            return Ok(());
        }
    }
    let r = set_generic_password(SERVICE, ACCOUNT, plain.as_bytes())
        .map_err(|e| format!("키체인 저장 실패({})", e.code()))
        .and_then(|_| match get_generic_password(SERVICE, ACCOUNT) {
            Ok(v) if v == plain.as_bytes() => Ok(()),
            Ok(_) => Err("키체인 저장 확인 실패(값 다름)".to_string()),
            Err(e) => Err(format!("키체인 저장 확인 실패({})", e.code())),
        });
    match &r {
        Ok(()) => set_note(None),
        Err(_) => set_note(Some("키체인에 저장하지 못함 — 지금은 주고받지만 앱을 다시 켜면 다시 넣어야 합니다".into())),
    }
    r
}

/// 읽기. 항목이 없으면 Ok(None).
pub fn load() -> Result<Option<String>, String> {
    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(v) => String::from_utf8(v).map(Some).map_err(|_| "키체인 값이 글자가 아님".to_string()),
        Err(e) if e.code() == NOT_FOUND => Ok(None),
        Err(e) => {
            set_note(Some("키체인에서 읽지 못함 — 암호문구를 다시 넣어 주세요".into()));
            Err(format!("키체인 읽기 실패({})", e.code()))
        }
    }
}

pub fn delete() {
    let _ = delete_generic_password(SERVICE, ACCOUNT);
}

/// 설정 파일 읽기·쓰기용 코덱(Windows `dpapi::with_codec` 와 같은 모양).
pub fn with_codec<R>(f: impl FnOnce(&logiflow_core::config::SecretCodec<'_>) -> R) -> R {
    let p = |s: &str| store(s).ok().map(|_| MARK.to_string());
    let u = |m: &str| if m == MARK { load().ok().flatten() } else { None };
    let codec = logiflow_core::config::SecretCodec { protect: &p, unprotect: &u };
    f(&codec)
}

/// `--keychain-selftest` — CI: 시험 항목을 쓰고 읽고 지운다(실제 암호문구 항목은 건드리지 않음). 0 = 됨.
pub fn selftest() -> i32 {
    const T: &str = "selftest";
    let v = "LogiFlow 키체인 시험 ✓";
    let w = set_generic_password(SERVICE, T, v.as_bytes());
    let r = get_generic_password(SERVICE, T);
    let d = delete_generic_password(SERVICE, T);
    let gone = matches!(get_generic_password(SERVICE, T), Err(e) if e.code() == NOT_FOUND);
    println!(
        "keychain write={:?} read={} delete={:?} gone={}",
        w.as_ref().err().map(|e| e.code()),
        matches!(&r, Ok(b) if b == v.as_bytes()),
        d.as_ref().err().map(|e| e.code()),
        gone
    );
    if w.is_ok() && matches!(&r, Ok(b) if b == v.as_bytes()) && d.is_ok() && gone {
        0
    } else {
        1
    }
}
