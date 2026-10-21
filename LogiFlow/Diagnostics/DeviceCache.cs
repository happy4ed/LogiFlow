using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;
using LogiFlow.Config;
using LogiFlow.Hid;

namespace LogiFlow.Diagnostics
{
    /// <summary>
    /// 스캔 결과를 %APPDATA%\LogiFlow\devices.txt 에 캐시한다. 시작 시 캐시로 바로 표시하고,
    /// 매번 재검색하지 않는다. 「새로 고침」·기기 변화 때만 재스캔. serial 등 민감값은 저장 안 함.
    /// </summary>
    internal static class DeviceCache
    {
        private static string CachePath
        {
            get { return Path.Combine(AppConfig.ConfigDirectory, "devices.txt"); }
        }

        public static void Save(List<ScanResult> results)
        {
            try
            {
                Directory.CreateDirectory(AppConfig.ConfigDirectory);
                var sb = new StringBuilder();
                sb.AppendLine("# LogiFlow 기기 캐시 v1");
                foreach (var r in results)
                {
                    // name \t vidpid \t conn \t devIdx \t chSupported \t chIndex \t hostCount \t curHost \t path \t outLen \t inLen \t usagePage \t note
                    sb.AppendLine(string.Join("\t", new[]
                    {
                        Escape(r.DeviceName), Escape(r.VidPid), Escape(r.ConnectionType),
                        r.DeviceIndex.ToString(CultureInfo.InvariantCulture),
                        r.ChangeHostSupported ? "1" : "0",
                        r.ChangeHostIndex.ToString(CultureInfo.InvariantCulture),
                        r.HostCount.ToString(CultureInfo.InvariantCulture),
                        r.CurrentHost.ToString(CultureInfo.InvariantCulture),
                        Escape(r.Interface != null ? r.Interface.Path : ""),
                        (r.Interface != null ? r.Interface.OutputReportByteLength : 0).ToString(CultureInfo.InvariantCulture),
                        (r.Interface != null ? r.Interface.InputReportByteLength : 0).ToString(CultureInfo.InvariantCulture),
                        (r.Interface != null ? r.Interface.UsagePage : 0).ToString(CultureInfo.InvariantCulture),
                        Escape(r.Note ?? ""),
                    }));
                }
                File.WriteAllText(CachePath, sb.ToString(), Encoding.UTF8);
            }
            catch
            {
            }
        }

        public static List<ScanResult> Load()
        {
            var list = new List<ScanResult>();
            try
            {
                if (!File.Exists(CachePath)) return list;
                foreach (string raw in File.ReadAllLines(CachePath))
                {
                    if (raw.Length == 0 || raw.StartsWith("#")) continue;
                    string[] f = raw.Split('\t');
                    if (f.Length < 13) continue;
                    var r = new ScanResult
                    {
                        DeviceName = Unescape(f[0]),
                        VidPid = Unescape(f[1]),
                        ConnectionType = Unescape(f[2]),
                        DeviceIndex = (byte)ParseInt(f[3]),
                        ChangeHostSupported = f[4] == "1",
                        ChangeHostIndex = (byte)ParseInt(f[5]),
                        HostCount = ParseInt(f[6]),
                        CurrentHost = ParseInt(f[7]),
                        Note = Unescape(f[12]),
                    };
                    string path = Unescape(f[8]);
                    if (!string.IsNullOrEmpty(path))
                    {
                        r.Interface = new HidInterfaceInfo
                        {
                            Path = path,
                            OutputReportByteLength = ParseInt(f[9]),
                            InputReportByteLength = ParseInt(f[10]),
                            UsagePage = (ushort)ParseInt(f[11]),
                            CapsRead = true,
                        };
                    }
                    list.Add(r);
                }
            }
            catch
            {
            }
            return list;
        }

        private static int ParseInt(string s)
        {
            int v;
            return int.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture, out v) ? v : 0;
        }

        private static string Escape(string s)
        {
            if (string.IsNullOrEmpty(s)) return "";
            return s.Replace("\\", "\\\\").Replace("\t", "\\t").Replace("\r", "").Replace("\n", " ");
        }

        private static string Unescape(string s)
        {
            if (string.IsNullOrEmpty(s)) return s;
            return s.Replace("\\t", "\t").Replace("\\\\", "\\");
        }
    }
}
