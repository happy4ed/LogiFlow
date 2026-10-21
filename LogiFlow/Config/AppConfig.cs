using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;
using LogiFlow.Handoff;
using LogiFlow.Ui;

namespace LogiFlow.Config
{
    /// <summary>엣지 변 → 채널 매핑 1건.</summary>
    public sealed class EdgeAttachment
    {
        public int Channel;             // 0-base 호스트 인덱스(채널 N → Channel = N-1)
        public string MonitorDeviceName; // Screen.DeviceName (예: \\.\DISPLAY1)
        public HandoffEdge Edge;
    }

    /// <summary>
    /// 앱 설정. %APPDATA%\LogiFlow\config.txt 에 간단한 key=value 형식으로 저장.
    /// </summary>
    public sealed class AppConfig
    {
        public bool EdgeSwitchEnabled = true;    // 엣지 자동 전환 켬(기본). 배치 비면 아무 동작 없음.
        public int DwellMs = EdgeDetector.DefaultDwellMs;
        public bool AutoStart = true;
        public bool DragGuard = true;            // 드래그 중 전환 안 함(디텍터가 buttonDown 무시).
        public bool DiagLogEnabled = true;       // 진단 로그 남기기.
        public bool PreLogon = false;            // 로그온 전에도 실행(고급·선택). 실제 상태는 PreLogon.IsInstalled().
        public int MyHost = -1;                   // 이 PC 의 호스트(채널) 인덱스 0-base. 로그온 전 '당기기' 대상.
        public int FontOffset = 1;                // 글자 크기 단계(pt 가산). 작게 -1 · 보통 +1 · 크게 +3.
        public ThemeMode Theme = ThemeMode.System;
        public readonly List<EdgeAttachment> Attachments = new List<EdgeAttachment>();
        public readonly Dictionary<int, string> Aliases = new Dictionary<int, string>(); // 채널(0-base)→별칭

        public string AliasFor(int channel)
        {
            string v;
            if (Aliases.TryGetValue(channel, out v) && !string.IsNullOrEmpty(v))
            {
                return v;
            }
            return string.Format("채널 {0}", channel + 1);
        }

        public static string ConfigDirectory
        {
            get
            {
                return Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                    "LogiFlow");
            }
        }

        public static string ConfigPath
        {
            get { return Path.Combine(ConfigDirectory, "config.txt"); }
        }

        public static AppConfig Load()
        {
            var cfg = new AppConfig();
            try
            {
                if (!File.Exists(ConfigPath))
                {
                    return cfg;
                }
                foreach (string raw in File.ReadAllLines(ConfigPath))
                {
                    string line = raw.Trim();
                    if (line.Length == 0 || line.StartsWith("#")) continue;
                    int eq = line.IndexOf('=');
                    if (eq < 0) continue;
                    string key = line.Substring(0, eq).Trim();
                    string val = line.Substring(eq + 1).Trim();
                    switch (key)
                    {
                        case "edgeEnabled": cfg.EdgeSwitchEnabled = val == "1"; break;
                        case "dwellMs":
                            int d;
                            if (int.TryParse(val, NumberStyles.Integer, CultureInfo.InvariantCulture, out d) && d >= 50 && d <= 3000)
                                cfg.DwellMs = d;
                            break;
                        case "autostart": cfg.AutoStart = val == "1"; break;
                        case "dragGuard": cfg.DragGuard = val == "1"; break;
                        case "diagLog": cfg.DiagLogEnabled = val == "1"; break;
                        case "preLogon": cfg.PreLogon = val == "1"; break;
                        case "myHost":
                            int mh;
                            if (int.TryParse(val, out mh) && mh >= -1 && mh <= 15) cfg.MyHost = mh;
                            break;
                        case "fontOffset":
                            int fo;
                            if (int.TryParse(val, out fo) && fo >= -2 && fo <= 6) cfg.FontOffset = fo;
                            break;
                        case "theme":
                            int tm;
                            if (int.TryParse(val, out tm) && tm >= 0 && tm <= 2) cfg.Theme = (ThemeMode)tm;
                            break;
                        case "alias":
                            string[] ap = val.Split('\t');
                            int ac;
                            if (ap.Length >= 2 && int.TryParse(ap[0], out ac)) cfg.Aliases[ac] = ap[1];
                            break;
                        case "attach":
                            EdgeAttachment a = ParseAttach(val);
                            if (a != null) cfg.Attachments.Add(a);
                            break;
                    }
                }
            }
            catch
            {
                // 설정 읽기 실패 시 기본값.
            }
            return cfg;
        }

        private static EdgeAttachment ParseAttach(string val)
        {
            // 형식: <channel>\t<monitorDeviceName>\t<edge>
            string[] parts = val.Split('\t');
            if (parts.Length < 3) return null;
            int channel, edge;
            if (!int.TryParse(parts[0], out channel)) return null;
            if (!int.TryParse(parts[2], out edge)) return null;
            if (edge < 0 || edge > 3) return null;
            return new EdgeAttachment
            {
                Channel = channel,
                MonitorDeviceName = parts[1],
                Edge = (HandoffEdge)edge,
            };
        }

        public void Save()
        {
            try
            {
                Directory.CreateDirectory(ConfigDirectory);
                var sb = new StringBuilder();
                sb.AppendLine("# LogiFlow 설정");
                sb.AppendLine("edgeEnabled=" + (EdgeSwitchEnabled ? "1" : "0"));
                sb.AppendLine("dwellMs=" + DwellMs.ToString(CultureInfo.InvariantCulture));
                sb.AppendLine("autostart=" + (AutoStart ? "1" : "0"));
                sb.AppendLine("dragGuard=" + (DragGuard ? "1" : "0"));
                sb.AppendLine("diagLog=" + (DiagLogEnabled ? "1" : "0"));
                sb.AppendLine("preLogon=" + (PreLogon ? "1" : "0"));
                sb.AppendLine("myHost=" + MyHost.ToString(CultureInfo.InvariantCulture));
                sb.AppendLine("fontOffset=" + FontOffset.ToString(CultureInfo.InvariantCulture));
                sb.AppendLine("theme=" + ((int)Theme).ToString(CultureInfo.InvariantCulture));
                foreach (var kv in Aliases)
                {
                    if (!string.IsNullOrEmpty(kv.Value))
                        sb.AppendLine(string.Format("alias={0}\t{1}", kv.Key, kv.Value));
                }
                foreach (var a in Attachments)
                {
                    sb.AppendLine(string.Format("attach={0}\t{1}\t{2}", a.Channel, a.MonitorDeviceName, (int)a.Edge));
                }
                File.WriteAllText(ConfigPath, sb.ToString(), Encoding.UTF8);
            }
            catch
            {
                // 저장 실패는 무시(진단 로그는 호출 측에서).
            }
        }
    }
}
