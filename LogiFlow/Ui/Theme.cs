using System;
using System.Drawing;
using Microsoft.Win32;

namespace LogiFlow.Ui
{
    public enum ThemeMode
    {
        System = 0,
        Light = 1,
        Dark = 2
    }

    /// <summary>
    /// 디자인 brief v3 토큰(Light/Dark)을 담은 테마. 모드=시스템이면 Windows 설정을 따른다.
    /// 색은 승인 시안 styles.css 값과 일치.
    /// </summary>
    public static class Theme
    {
        private static ThemeMode _mode = ThemeMode.System;

        public static event EventHandler Changed;

        public static ThemeMode Mode { get { return _mode; } }

        public static void SetMode(ThemeMode mode)
        {
            _mode = mode;
            var h = Changed;
            if (h != null) h(null, EventArgs.Empty);
        }

        public static bool IsDark
        {
            get
            {
                switch (_mode)
                {
                    case ThemeMode.Light: return false;
                    case ThemeMode.Dark: return true;
                    default: return SystemIsDark();
                }
            }
        }

        private static bool SystemIsDark()
        {
            try
            {
                using (RegistryKey key = Registry.CurrentUser.OpenSubKey(
                    @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", false))
                {
                    if (key == null) return false;
                    object v = key.GetValue("AppsUseLightTheme");
                    if (v is int)
                    {
                        return (int)v == 0; // 0 = dark
                    }
                }
            }
            catch
            {
            }
            return false;
        }

        private static Color Pick(string light, string dark)
        {
            return Hex(IsDark ? dark : light);
        }

        public static Color Hex(string hex)
        {
            hex = hex.TrimStart('#');
            int r = Convert.ToInt32(hex.Substring(0, 2), 16);
            int g = Convert.ToInt32(hex.Substring(2, 2), 16);
            int b = Convert.ToInt32(hex.Substring(4, 2), 16);
            return Color.FromArgb(r, g, b);
        }

        // ---- 토큰 (styles.css brief v3) ----
        public static Color Content { get { return Pick("FAFAFA", "202022"); } }
        public static Color Section { get { return Pick("FFFFFF", "29292C"); } }
        public static Color Sidebar { get { return Pick("EFEFF1", "252527"); } }
        public static Color Ink { get { return Pick("222226", "ECECEF"); } }
        public static Color Muted { get { return Pick("6F6F76", "A4A4AA"); } }
        public static Color Border { get { return Pick("E2E2E6", "3A3A3E"); } }
        public static Color Accent { get { return Pick("007AFF", "0A84FF"); } }
        public static Color AccentLine { get { return Pick("007AFF", "409CFF"); } }
        public static Color Chip { get { return Pick("E7E7EB", "38383D"); } }
        public static Color Field { get { return Pick("FFFFFF", "303034"); } }
        public static Color Hover { get { return Pick("F1F1F4", "313135"); } }
        public static Color ToggleOff { get { return Pick("D2D2D8", "48484E"); } }
        public static Color Ok { get { return Pick("30B94E", "30D158"); } }
        public static Color Divider { get { return Pick("ECECEF", "343438"); } }

        public static Color MonFill { get { return Pick("FFFFFF", "303034"); } }
        public static Color MonStroke { get { return Pick("CFCFD6", "4A4A50"); } }
        public static Color CanvasBg { get { return Pick("F6F6F8", "232326"); } }
        public static Color CanvasGrid { get { return Pick("ECECEF", "2E2E32"); } }
        public static Color PcManualFill { get { return Pick("ECECEF", "313135"); } }
        public static Color PcManualStroke { get { return Pick("BFBFC6", "4A4A50"); } }

        /// <summary>accent 를 바탕색 위에 알파 혼합한 근사치(soft).</summary>
        public static Color AccentSoft
        {
            get { return Blend(Accent, Content, IsDark ? 0.20 : 0.12); }
        }

        public static Color MonPrimaryFill
        {
            get { return Blend(Accent, MonFill, IsDark ? 0.20 : 0.08); }
        }

        public static Color PcAutoFill
        {
            get { return Blend(Accent, CanvasBg, IsDark ? 0.22 : 0.10); }
        }

        public static Color Blend(Color fg, Color bg, double alpha)
        {
            int r = (int)(fg.R * alpha + bg.R * (1 - alpha));
            int g = (int)(fg.G * alpha + bg.G * (1 - alpha));
            int b = (int)(fg.B * alpha + bg.B * (1 - alpha));
            return Color.FromArgb(Clamp(r), Clamp(g), Clamp(b));
        }

        private static int Clamp(int v)
        {
            if (v < 0) return 0;
            if (v > 255) return 255;
            return v;
        }
    }
}
