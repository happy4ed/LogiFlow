using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;
using System.Windows.Forms;

namespace LogiFlow.Config
{
    /// <summary>로그온 화면 전환 워커(SYSTEM)가 읽는 엣지 설정 1건.</summary>
    public sealed class LogonEdge
    {
        public string Device;   // Screen.DeviceName
        public int Index;       // 보조 매칭용 모니터 인덱스
        public int Edge;        // HandoffEdge (Left0/Right1/Top2/Bottom3)
        public int Channel;     // 0-base 대상 채널
    }

    /// <summary>
    /// 로그온/잠금 화면 전환용 '머신 공유' 엣지 설정(%ProgramData%\LogiFlow\logon-edges.txt).
    /// 사용자 세션 앱이 쓰고(SYSTEM 이 읽을 수 있게 ProgramData), winlogon 워커가 읽는다.
    /// 좌표는 저장하지 않고 모니터 식별(이름/인덱스)만 저장 — 워커가 자기 Screen.AllScreens 로 경계를 구해
    /// 자기 GetCursorPos 와 같은 좌표계에서 판정한다(크로스 프로세스 DPI 불일치 방지).
    /// </summary>
    public static class LogonEdges
    {
        public static string Dir
        {
            get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "LogiFlow"); }
        }
        public static string FilePath { get { return Path.Combine(Dir, "logon-edges.txt"); } }

        public static void Write(AppConfig cfg)
        {
            try
            {
                var screens = Screen.AllScreens;
                var sb = new StringBuilder();
                sb.AppendLine("dwellMs=" + cfg.DwellMs.ToString(CultureInfo.InvariantCulture));
                sb.AppendLine("myHost=" + cfg.MyHost.ToString(CultureInfo.InvariantCulture));
                foreach (var a in cfg.Attachments)
                {
                    int idx = -1;
                    for (int i = 0; i < screens.Length; i++)
                        if (screens[i].DeviceName == a.MonitorDeviceName) { idx = i; break; }
                    sb.AppendLine(string.Format("edge\t{0}\t{1}\t{2}\t{3}", a.MonitorDeviceName, idx, (int)a.Edge, a.Channel));
                }
                Directory.CreateDirectory(Dir);
                File.WriteAllText(FilePath, sb.ToString(), Encoding.UTF8);
            }
            catch { }
        }

        public static void Remove()
        {
            try { if (File.Exists(FilePath)) File.Delete(FilePath); } catch { }
        }

        public static List<LogonEdge> Read(out int dwellMs, out int myHost)
        {
            dwellMs = 330; myHost = -1;
            var list = new List<LogonEdge>();
            try
            {
                if (!File.Exists(FilePath)) return list;
                foreach (var raw in File.ReadAllLines(FilePath))
                {
                    string line = raw.Trim();
                    if (line.Length == 0) continue;
                    if (line.StartsWith("dwellMs=")) { int d; if (int.TryParse(line.Substring(8), out d)) dwellMs = d; continue; }
                    if (line.StartsWith("myHost=")) { int m; if (int.TryParse(line.Substring(7), out m)) myHost = m; continue; }
                    if (line.StartsWith("edge\t"))
                    {
                        string[] p = line.Split('\t');
                        if (p.Length >= 5)
                        {
                            int ix = -1, ed = 0, ch = 0;
                            int.TryParse(p[2], out ix); int.TryParse(p[3], out ed); int.TryParse(p[4], out ch);
                            list.Add(new LogonEdge { Device = p[1], Index = ix, Edge = ed, Channel = ch });
                        }
                    }
                }
            }
            catch { }
            return list;
        }
    }
}
