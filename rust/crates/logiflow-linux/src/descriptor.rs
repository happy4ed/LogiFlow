//! 리포트 서술자에서 HID++ 흔적만 읽는다 — C# `HidEnumerator.ParseDescriptor` 이식(OS 무관, 시험 가능).
//! 벤더 usage page(0xFF00·0xFF43) 아래 Report ID 0x10(short·7바이트)·0x11(long·20바이트).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HidppCaps {
    pub usage_page: u16,
    pub usage: u16,
    pub report_len: usize,
}

pub fn parse(d: &[u8]) -> HidppCaps {
    let (mut page, mut vendor_page, mut vendor_usage) = (0u16, 0u16, 0u16);
    let (mut has_short, mut has_long) = (false, false);
    let mut i = 0usize;
    while i < d.len() {
        let b = d[i];
        if b == 0xFE {
            // long item: FE size tag data...
            if i + 1 >= d.len() {
                break;
            }
            i += 3 + d[i + 1] as usize;
            continue;
        }
        let mut size = (b & 0x3) as usize;
        if size == 3 {
            size = 4;
        }
        if i + size >= d.len() {
            break;
        }
        let mut val = 0u32;
        for k in 0..size {
            val |= (d[i + 1 + k] as u32) << (8 * k);
        }
        match b & 0xFC {
            0x04 => {
                page = val as u16;
                if page >= 0xFF00 && vendor_page == 0 {
                    vendor_page = page;
                }
            }
            0x08 => {
                if page >= 0xFF00 && vendor_usage == 0 {
                    vendor_usage = val as u16;
                }
            }
            0x84 if page >= 0xFF00 => {
                if val == 0x10 {
                    has_short = true;
                }
                if val == 0x11 {
                    has_long = true;
                }
            }
            _ => {}
        }
        i += 1 + size;
    }
    HidppCaps {
        usage_page: vendor_page,
        usage: if has_long {
            0x0202
        } else if has_short {
            0x0201
        } else {
            vendor_usage
        },
        report_len: if has_long {
            20
        } else if has_short {
            7
        } else {
            0
        },
    }
}

pub fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace().filter_map(|t| u8::from_str_radix(t, 16).ok()).collect()
}

/// 시험 서술자 — C# selftest 와 같은 셋(리시버 · 블루투스 · 일반 마우스).
pub const RECEIVER: &str = "06 00 FF 09 01 A1 01 85 10 75 08 95 06 15 00 26 FF 00 09 01 81 00 09 01 91 00 C0 \
06 00 FF 09 02 A1 01 85 11 75 08 95 13 15 00 26 FF 00 09 02 81 00 09 02 91 00 C0 \
06 00 FF 09 04 A1 01 85 20 75 08 95 0E 15 00 26 FF 00 09 41 81 00 09 41 91 00 \
85 21 95 1F 15 00 26 FF 00 09 42 81 00 09 42 91 00 C0";
pub const BLUETOOTH: &str = "05 01 09 02 A1 01 85 02 09 01 A1 00 05 09 19 01 29 10 15 00 25 01 95 10 75 01 81 02 C0 C0 \
06 43 FF 0A 02 02 A1 01 85 11 75 08 95 13 15 00 26 FF 00 09 02 81 00 09 02 91 00 C0";
pub const MOUSE: &str = "05 01 09 02 A1 01 85 10 09 01 A1 00 05 09 19 01 29 05 15 00 25 01 95 05 75 01 81 02 C0 C0";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_cases_match_csharp_selftest() {
        assert_eq!(
            parse(&hex(RECEIVER)),
            HidppCaps {
                usage_page: 0xFF00,
                usage: 0x0202,
                report_len: 20
            }
        );
        assert_eq!(
            parse(&hex(BLUETOOTH)),
            HidppCaps {
                usage_page: 0xFF43,
                usage: 0x0202,
                report_len: 20
            }
        );
        assert_eq!(
            parse(&hex(MOUSE)),
            HidppCaps {
                usage_page: 0,
                usage: 0,
                report_len: 0
            }
        );
    }
}
