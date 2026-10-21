using System;
using System.Drawing;
using System.Drawing.Drawing2D;

namespace LogiFlow.Ui
{
    /// <summary>
    /// 승인 시안 icons.js 의 라인 아이콘을 GDI 로 근사 렌더. 좌표는 24-unit viewBox 기준.
    /// </summary>
    public static class Icons
    {
        private static PointF P(RectangleF r, float x, float y)
        {
            return new PointF(r.X + x / 24f * r.Width, r.Y + y / 24f * r.Height);
        }

        private static float U(RectangleF r)
        {
            return Math.Min(r.Width, r.Height) / 24f;
        }

        private static Pen MakePen(Color c, RectangleF r, float width)
        {
            var p = new Pen(c, Math.Max(1f, width * U(r)));
            p.StartCap = LineCap.Round;
            p.EndCap = LineCap.Round;
            p.LineJoin = LineJoin.Round;
            return p;
        }

        private static void Rounded(Graphics g, Pen pen, RectangleF r, float x, float y, float w, float h, float rad)
        {
            float u = U(r);
            var rect = new RectangleF(r.X + x * u, r.Y + y * u, w * u, h * u);
            float d = rad * u * 2;
            using (var path = new GraphicsPath())
            {
                path.AddArc(rect.X, rect.Y, d, d, 180, 90);
                path.AddArc(rect.Right - d, rect.Y, d, d, 270, 90);
                path.AddArc(rect.Right - d, rect.Bottom - d, d, d, 0, 90);
                path.AddArc(rect.X, rect.Bottom - d, d, d, 90, 90);
                path.CloseFigure();
                g.DrawPath(pen, path);
            }
        }

        public static void Mark(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.9f))
            using (var brush = new SolidBrush(c))
            {
                Rounded(g, pen, r, 2.3f, 9.2f, 8.3f, 7.8f, 2f);
                Rounded(g, pen, r, 13.4f, 9.2f, 8.3f, 7.8f, 2f);
                using (var path = new GraphicsPath())
                {
                    path.AddBezier(P(r, 8.6f, 9.2f), P(r, 9.4f, 4.9f), P(r, 14.6f, 4.9f), P(r, 15.4f, 9.2f));
                    g.DrawPath(pen, path);
                }
                g.FillPolygon(brush, new[] { P(r, 15.7f, 9.6f), P(r, 13.1f, 8.7f), P(r, 14.4f, 11.1f) });
            }
        }

        public static void Swap(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                g.DrawLines(pen, new[] { P(r, 4, 8), P(r, 17, 8) });
                g.DrawLines(pen, new[] { P(r, 13, 4), P(r, 17, 8), P(r, 13, 12) });
                g.DrawLines(pen, new[] { P(r, 20, 16), P(r, 7, 16) });
                g.DrawLines(pen, new[] { P(r, 11, 20), P(r, 7, 16), P(r, 11, 12) });
            }
        }

        public static void Gear(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                float u = U(r);
                var p = P(r, 12, 12);
                float rad = 3.2f * u;
                g.DrawEllipse(pen, p.X - rad, p.Y - rad, rad * 2, rad * 2);
                for (int i = 0; i < 8; i++)
                {
                    double a = i * Math.PI / 4;
                    var a1 = new PointF(p.X + (float)Math.Cos(a) * 5f * u, p.Y + (float)Math.Sin(a) * 5f * u);
                    var a2 = new PointF(p.X + (float)Math.Cos(a) * 8f * u, p.Y + (float)Math.Sin(a) * 8f * u);
                    g.DrawLine(pen, a1, a2);
                }
            }
        }

        public static void Diag(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                g.DrawLines(pen, new[] { P(r, 3, 12), P(r, 6, 12), P(r, 8.5f, 18), P(r, 12.5f, 5), P(r, 15, 12), P(r, 21, 12) });
            }
        }

        public static void Refresh(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                float u = U(r);
                var ctr = P(r, 12, 12);
                g.DrawArc(pen, ctr.X - 8 * u, ctr.Y - 8 * u, 16 * u, 16 * u, -60, 300);
                g.DrawLines(pen, new[] { P(r, 20, 4), P(r, 20, 9), P(r, 15, 9) });
            }
        }

        public static void Folder(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                using (var path = new GraphicsPath())
                {
                    PointF[] pts =
                    {
                        P(r, 3, 7), P(r, 3, 17), P(r, 21, 17), P(r, 21, 9), P(r, 11, 9), P(r, 9, 7)
                    };
                    path.AddLines(pts);
                    path.CloseFigure();
                    g.DrawPath(pen, path);
                }
            }
        }

        public static void Plug(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                g.DrawLine(pen, P(r, 9, 3), P(r, 9, 8));
                g.DrawLine(pen, P(r, 15, 3), P(r, 15, 8));
                using (var path = new GraphicsPath())
                {
                    path.AddLine(P(r, 6.5f, 8), P(r, 17.5f, 8));
                    path.AddLine(P(r, 17.5f, 8), P(r, 17.5f, 11));
                    path.AddArc(P(r, 6.5f, 8).X, P(r, 8, 8).Y, 11 * U(r), 11 * U(r), 0, 180);
                    g.DrawPath(pen, path);
                }
                g.DrawLine(pen, P(r, 12, 16.5f), P(r, 12, 21));
            }
        }

        public static void Mouse(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                Rounded(g, pen, r, 7, 3, 10, 18, 5f);
                g.DrawLine(pen, P(r, 12, 7), P(r, 12, 10));
            }
        }

        public static void Keyboard(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                Rounded(g, pen, r, 2.5f, 6.5f, 19, 11, 2f);
                float[] xs = { 6, 9, 12, 15, 18 };
                foreach (float x in xs)
                {
                    var pt = P(r, x, 10);
                    g.DrawLine(pen, pt, new PointF(pt.X + 0.4f, pt.Y));
                }
                g.DrawLine(pen, P(r, 7, 14), P(r, 17, 14));
            }
        }

        public static void Pencil(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                using (var path = new GraphicsPath())
                {
                    path.AddLines(new[] { P(r, 4, 20), P(r, 8, 20), P(r, 19, 9), P(r, 15, 5), P(r, 4, 16) });
                    path.CloseFigure();
                    g.DrawPath(pen, path);
                }
                g.DrawLine(pen, P(r, 14, 6), P(r, 18, 10));
            }
        }

        public static void Edge(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                Rounded(g, pen, r, 3, 4, 13, 10, 1.6f);
                g.DrawLine(pen, P(r, 16, 9), P(r, 21, 9));
                g.DrawLines(pen, new[] { P(r, 18, 6), P(r, 21, 9), P(r, 18, 12) });
            }
        }

        public static void Monitor(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                Rounded(g, pen, r, 3, 4, 18, 12, 2f);
                g.DrawLine(pen, P(r, 8, 20), P(r, 16, 20));
                g.DrawLine(pen, P(r, 12, 16), P(r, 12, 20));
            }
        }

        public static void Power(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                g.DrawLine(pen, P(r, 12, 4), P(r, 12, 12));
                g.DrawArc(pen, P(r, 5, 5).X, P(r, 5, 5).Y, 14 * U(r), 14 * U(r), -60, 300);
            }
        }

        public static void Check(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 2.4f))
            {
                g.DrawLines(pen, new[] { P(r, 5, 12), P(r, 9.5f, 16.5f), P(r, 19, 7) });
            }
        }

        public static void Chevron(Graphics g, RectangleF r, Color c)
        {
            using (var pen = MakePen(c, r, 1.7f))
            {
                g.DrawLines(pen, new[] { P(r, 6, 9), P(r, 12, 15), P(r, 18, 9) });
            }
        }
    }
}
