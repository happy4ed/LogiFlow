using System;
using System.Collections.Generic;
using System.Drawing;

namespace LogiFlow.Ui
{
    /// <summary>
    /// 앱 전역 폰트. 최소 크기(≈12.7px ≥ 12px)를 보장하고, 사용자 글자 크기 단계를
    /// 기준 pt 에 더하는 방식으로 적용한다(작게 -1 · 보통 +1 · 크게 +3 pt).
    /// 가산을 낮춰도 최소 미만으로는 내려가지 않는다.
    /// </summary>
    public static class UiFont
    {
        public const float MinPt = 9.5f;   // ≈ 12.7px @96dpi, 최소 가독 크기
        private const string Family = "Segoe UI";

        private static float _offset = 1f;  // 기본 '보통' = 기존 크기 +1pt
        private static readonly Dictionary<string, Font> _cache = new Dictionary<string, Font>();

        public static event EventHandler Changed;

        public static float Offset { get { return _offset; } }

        /// <summary>글자 크기 단계를 pt 가산으로 설정(작게 -1 · 보통 +1 · 크게 +3).</summary>
        public static void SetOffset(float offsetPt)
        {
            if (offsetPt < -2f) offsetPt = -2f;
            if (offsetPt > 6f) offsetPt = 6f;
            _offset = offsetPt;
            var h = Changed;
            if (h != null) h(null, EventArgs.Empty);
        }

        /// <summary>기준 pt 에 가산·최소크기 적용해 Font 반환(인스턴스 캐시·프로세스 수명).</summary>
        public static Font Get(float basePt, FontStyle style)
        {
            float pt = basePt + _offset;
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
