//! 디자인 brief v3 토큰(Light/Dark) — C# `Ui/Theme.cs` 값 그대로(승인 시안 styles.css 와 일치).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// C# Theme.Blend — fg 를 bg 위에 alpha 로 섞고 정수로 자른다(반올림 아님).
pub fn blend(fg: Rgb, bg: Rgb, alpha: f64) -> Rgb {
    let mix = |f: u8, b: u8| -> u8 {
        let v = (f as f64 * alpha + b as f64 * (1.0 - alpha)) as i32;
        v.clamp(0, 255) as u8
    };
    Rgb(mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2))
}

const fn hex(v: u32) -> Rgb {
    Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// 한 모드의 토큰 묶음.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub content: Rgb,
    pub section: Rgb,
    pub sidebar: Rgb,
    pub ink: Rgb,
    pub muted: Rgb,
    pub border: Rgb,
    pub accent: Rgb,
    pub accent_line: Rgb,
    pub chip: Rgb,
    pub field: Rgb,
    pub hover: Rgb,
    pub toggle_off: Rgb,
    pub ok: Rgb,
    /// 주의 글자(값이 비었음·막힘 등) — rs.3 추가. 흰 바탕 5.2:1 · 다크 카드 7.1:1.
    pub warn: Rgb,
    pub divider: Rgb,
    pub mon_fill: Rgb,
    pub mon_stroke: Rgb,
    pub canvas_bg: Rgb,
    pub canvas_grid: Rgb,
    pub pc_manual_fill: Rgb,
    pub pc_manual_stroke: Rgb,
}

pub const LIGHT: Palette = Palette {
    dark: false,
    content: hex(0xFAFAFA),
    section: hex(0xFFFFFF),
    sidebar: hex(0xEFEFF1),
    ink: hex(0x222226),
    muted: hex(0x6F6F76),
    border: hex(0xE2E2E6),
    accent: hex(0x007AFF),
    accent_line: hex(0x007AFF),
    chip: hex(0xE7E7EB),
    field: hex(0xFFFFFF),
    hover: hex(0xF1F1F4),
    toggle_off: hex(0xD2D2D8),
    ok: hex(0x30B94E),
    warn: hex(0xB25000),
    divider: hex(0xECECEF),
    mon_fill: hex(0xFFFFFF),
    mon_stroke: hex(0xCFCFD6),
    canvas_bg: hex(0xF6F6F8),
    canvas_grid: hex(0xECECEF),
    pc_manual_fill: hex(0xECECEF),
    pc_manual_stroke: hex(0xBFBFC6),
};

pub const DARK: Palette = Palette {
    dark: true,
    content: hex(0x202022),
    section: hex(0x29292C),
    sidebar: hex(0x252527),
    ink: hex(0xECECEF),
    muted: hex(0xA4A4AA),
    border: hex(0x3A3A3E),
    accent: hex(0x0A84FF),
    accent_line: hex(0x409CFF),
    chip: hex(0x38383D),
    field: hex(0x303034),
    hover: hex(0x313135),
    toggle_off: hex(0x48484E),
    ok: hex(0x30D158),
    warn: hex(0xFF9F0A),
    divider: hex(0x343438),
    mon_fill: hex(0x303034),
    mon_stroke: hex(0x4A4A50),
    canvas_bg: hex(0x232326),
    canvas_grid: hex(0x2E2E32),
    pc_manual_fill: hex(0x313135),
    pc_manual_stroke: hex(0x4A4A50),
};

impl Palette {
    pub fn of(dark: bool) -> &'static Palette {
        if dark {
            &DARK
        } else {
            &LIGHT
        }
    }
    /// accent 를 바탕색 위에 알파 혼합한 근사치(soft).
    pub fn accent_soft(&self) -> Rgb {
        blend(self.accent, self.content, if self.dark { 0.20 } else { 0.12 })
    }
    pub fn mon_primary_fill(&self) -> Rgb {
        blend(self.accent, self.mon_fill, if self.dark { 0.20 } else { 0.08 })
    }
    pub fn pc_auto_fill(&self) -> Rgb {
        blend(self.accent, self.canvas_bg, if self.dark { 0.22 } else { 0.10 })
    }
}
