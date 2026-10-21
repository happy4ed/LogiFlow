using System;
using System.Collections.Generic;
using System.Drawing;

namespace LogiFlow.Ui
{
    /// <summary>
    /// 앱 전역 폰트. 최소 크기(≈12.7px ≥ 12px)를 보장하고, 사용자 글자 크기(%)로 스케일한다.
    /// 스케일을 낮춰도 최소 미만으로는 내려가지 않는다.
    /// </summary>
    public static class UiFont
    {
        public const float MinPt = 9.5f;   // ≈ 12.7px @96dpi, 최소 가독 크기
        private const string Family = "Segoe UI";

        private static float _scale = 1.0f;
        private static readonly Dictionary<string, Font> _cache = new Dictionary<string, Font>();

        public static event EventHandler Changed;

        public static int Percent { get; private set; } = 100;

        public static void SetPercent(int pct)
        {
            if (pct < 80) pct = 80;
            if (pct > 160) pct = 160;
            Percent = pct;
            _scale = pct / 100f;
            var h = Changed;
            if (h != null) h(null, EventArgs.Empty);
        }

        /// <summary>기준 pt 를 스케일·최소크기 적용해 Font 반환(인스턴스 캐시·프로세스 수명).</summary>
        public static Font Get(float basePt, FontStyle style)
        {
            float pt = basePt * _scale;
            if (pt < MinPt) pt = MinPt;
            string key = pt.ToString("0.0") + "|" + (int)style;
            Font f;
            if (_cache.TryGetValue(key, out f)) return f;
            f = new Font(Family, pt, style);
            _cache[key] = f;
            return f;
        }

        public static Font Get(float basePt)
        {
            return Get(basePt, FontStyle.Regular);
        }
    }
}
