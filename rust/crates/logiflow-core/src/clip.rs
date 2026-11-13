//! 클립보드 공유 프로토콜 — C# `ClipboardSync` 과 바이트 호환.
//!
//! 프레임 = 길이(4바이트 little-endian) + [IV 16 | AES-256-CBC(PKCS7) 암호문 | HMAC-SHA256 32].
//! 키 = 암호문구(NFKC) → PBKDF2-HMAC-SHA1(소금 "LogiFlow-Clip-v1", 20000회) 64바이트 → 앞 32 AES · 뒤 32 HMAC.
//! 평문 = [종류 1바이트(0 텍스트 UTF-8 · 1 PNG) | 내용].

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_PORT: i32 = 48737;
const SALT: &[u8] = b"LogiFlow-Clip-v1";
const ITERATIONS: u32 = 20000;

pub const KIND_TEXT: u8 = 0;
pub const KIND_IMAGE: u8 = 1;

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type HmacSha256 = Hmac<Sha256>;

/// 전각(한글 IME) 문자를 반각으로 정규화 — IP·암호문구가 양쪽에서 일치하도록. C#: Trim → FormKC.
pub fn norm(s: &str) -> String {
    s.trim().nfkc().collect()
}

#[derive(Clone)]
pub struct ClipKeys {
    pub aes: [u8; 32],
    pub mac: [u8; 32],
}

pub fn derive_keys(pass: &str) -> ClipKeys {
    let mut out = [0u8; 64];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(norm(pass).as_bytes(), SALT, ITERATIONS, &mut out);
    let mut aes = [0u8; 32];
    let mut mac = [0u8; 32];
    aes.copy_from_slice(&out[..32]);
    mac.copy_from_slice(&out[32..]);
    ClipKeys { aes, mac }
}

/// IV 를 받아 암호화(시험용). 보통은 `encrypt`.
pub fn encrypt_with_iv(keys: &ClipKeys, plain: &[u8], iv: &[u8; 16]) -> Vec<u8> {
    let ct = Aes256CbcEnc::new(&keys.aes.into(), iv.into()).encrypt_padded_vec_mut::<Pkcs7>(plain);
    let mut body = Vec::with_capacity(16 + ct.len() + 32);
    body.extend_from_slice(iv);
    body.extend_from_slice(&ct);
    let mut m = <HmacSha256 as Mac>::new_from_slice(&keys.mac).expect("hmac key");
    m.update(&body);
    let tag = m.finalize().into_bytes();
    body.extend_from_slice(&tag);
    body
}

pub fn encrypt(keys: &ClipKeys, plain: &[u8]) -> Vec<u8> {
    let mut iv = [0u8; 16];
    getrandom::getrandom(&mut iv).expect("random iv");
    encrypt_with_iv(keys, plain, &iv)
}

/// 인증(HMAC) 실패·형식 오류면 None.
pub fn decrypt(keys: &ClipKeys, frame: &[u8]) -> Option<Vec<u8>> {
    if frame.len() < 16 + 32 {
        return None;
    }
    let body_len = frame.len() - 32;
    let (body, tag) = frame.split_at(body_len);
    let mut m = <HmacSha256 as Mac>::new_from_slice(&keys.mac).ok()?;
    m.update(body);
    m.verify_slice(tag).ok()?; // 상수 시간 비교
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&body[..16]);
    Aes256CbcDec::new(&keys.aes.into(), (&iv).into())
        .decrypt_padded_vec_mut::<Pkcs7>(&body[16..])
        .ok()
}

pub fn text_payload(text: &str) -> Vec<u8> {
    let mut p = Vec::with_capacity(text.len() + 1);
    p.push(KIND_TEXT);
    p.extend_from_slice(text.as_bytes());
    p
}

pub fn image_payload(png: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(png.len() + 1);
    p.push(KIND_IMAGE);
    p.extend_from_slice(png);
    p
}

/// 에코(핑퐁) 방지용 해시.
pub fn hash(data: &[u8]) -> String {
    let d = Sha256::digest(data);
    d.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join("-")
}

/// 길이 머리(4바이트 LE)를 붙인 전송 바이트.
pub fn wire(frame: &[u8]) -> Vec<u8> {
    let mut w = Vec::with_capacity(frame.len() + 4);
    w.extend_from_slice(&(frame.len() as i32).to_le_bytes());
    w.extend_from_slice(frame);
    w
}

/// 수신 길이 머리 검사 — C#: len <= 0 || len > MaxBytes + 4096 이면 버림.
pub fn valid_len(len: i32) -> bool {
    len > 0 && (len as i64) <= (MAX_BYTES as i64 + 4096)
}
