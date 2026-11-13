//! 클립보드 암호문구 보호 — Windows DPAPI(사용자 범위, CryptProtectData). 지금 판과 다른 점(평문 저장 → 보호 저장).
//! 같은 Windows 사용자만 풀 수 있다. 저장 형식은 hex 문자열(`clipSecretDpapi=`).

use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::*;

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

pub fn protect(plain: &str) -> Option<String> {
    let bytes = plain.as_bytes();
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(&input, PCWSTR::null(), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).ok()?;
        let data = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
        Some(to_hex(&data))
    }
}

pub fn unprotect(hex: &str) -> Option<String> {
    let enc = from_hex(hex)?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: enc.len() as u32,
        pbData: enc.as_ptr() as *mut u8,
    };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).ok()?;
        let data = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
        String::from_utf8(data).ok()
    }
}

/// 설정 파일 읽기·쓰기용 코덱.
pub fn with_codec<R>(f: impl FnOnce(&logiflow_core::config::SecretCodec<'_>) -> R) -> R {
    let p = |s: &str| protect(s);
    let u = |s: &str| unprotect(s);
    let codec = logiflow_core::config::SecretCodec { protect: &p, unprotect: &u };
    f(&codec)
}
