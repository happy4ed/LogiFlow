using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Reflection;
using LogiFlow.Diagnostics;
using LogiFlow.Hid;

namespace LogiFlow
{
    /// <summary>
    /// LogiFlow 리눅스 CLI — 터미널 명령으로 로지텍 Easy-Switch 채널 전환.
    /// 전환 쓰기는 윈도우판과 같은 HostSwitcher.SwitchAllBurst 한 경로(가드 동일).
    /// 기기 정보는 캐시해 두고 바로 보낸다(스캔은 처음·실패 때만).
    /// </summary>
    internal static class Program
    {
        private const ushort LogitechVid = 0x046D;

        private static int Main(string[] args)
        {
            bool debug = false, rescan = false;
            var rest = new List<string>();
            foreach (string a in args)
            {
                if (a == "--debug") debug = true;
                else if (a == "--rescan") rescan = true;
                else rest.Add(a);
            }

            string cmd = rest.Count > 0 ? rest[0] : "";
            int n;
            if (cmd == "" || cmd == "-h" || cmd == "--help" || cmd == "help") { Usage(); return cmd == "" ? 1 : 0; }
            if (cmd == "--version" || cmd == "version") { Console.WriteLine("logiflow " + Version()); return 0; }
            if (cmd == "selftest") return SelfTest();

            var raw = new DiagLogger(CacheDir()) { Enabled = debug };
            int code;
            if (cmd == "list") code = List(raw);
            else if (cmd == "switch" && rest.Count > 1 && TryChannel(rest[1], out n)) code = Switch(n, rescan, raw);
            else if (TryChannel(cmd, out n)) code = Switch(n, rescan, raw);
            else { Usage(); return 1; }

            if (debug) Console.WriteLine("HID 로그: " + raw.LogPath);
            return code;
        }

        private static void Usage()
        {
            Console.WriteLine("logiflow " + Version() + " — 로지텍 Easy-Switch 채널 전환(리눅스)");
            Console.WriteLine();
            Console.WriteLine("  logiflow switch <채널>   이 PC 에 붙은 마우스·키보드를 채널(1~3)로 넘김");
            Console.WriteLine("  logiflow <채널>          위와 같음");
            Console.WriteLine("  logiflow list            기기·현재 채널 보기(다시 찾음)");
            Console.WriteLine();
            Console.WriteLine("  --rescan   저장된 기기 정보 무시하고 다시 찾기");
            Console.WriteLine("  --debug    HID 송수신 로그 남기기");
            Console.WriteLine();
            Console.WriteLine("  기기가 지금 이 PC 에 연결돼 있을 때 다른 채널로 보내는 명령(당겨오기는 안 됨).");
        }

        private static bool TryChannel(string s, out int ch)
        {
            return int.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture, out ch) && ch >= 1 && ch <= 6;
        }

        private static int Switch(int channel, bool rescan, DiagLogger raw)
        {
            var nodes = HidEnumerator.Enumerate(LogitechVid, 0);
            int pre = Precheck(nodes);
            if (pre != 0) return pre;

            List<ScanResult> devs = rescan ? new List<ScanResult>() : LoadCache(nodes);
            bool fromCache = devs.Count > 0;
            if (!fromCache)
            {
                Console.WriteLine("기기 찾는 중…");
                devs = ScanAndSave(raw);
            }
            if (devs.Count == 0)
            {
                Console.Error.WriteLine("전환할 기기 없음 — 마우스·키보드가 지금 이 PC 에 연결돼 있어야 함");
                return 1;
            }

            int ok = Run(devs, channel, raw);
            if (ok == 0 && fromCache)
            {
                // 저장된 정보가 낡았을 수 있음(재페어링·다른 리시버) → 다시 찾아 한 번 더.
                Console.WriteLine("저장된 기기 정보로 실패 — 다시 찾는 중…");
                devs = ScanAndSave(raw);
                if (devs.Count > 0) ok = Run(devs, channel, raw);
            }
            return ok > 0 ? 0 : 1;
        }

        private static int Run(List<ScanResult> devs, int channel, DiagLogger raw)
        {
            Console.WriteLine(string.Format("채널 {0} 으로 전환 — 기기 {1}개", channel, devs.Count));
            int ok = HostSwitcher.SwitchAllBurst(devs, channel - 1, raw, s => Console.WriteLine(s), 3, null);
            Console.WriteLine(string.Format("결과: {0}/{1}개 전환", ok, devs.Count));
            return ok;
        }

        private static int List(DiagLogger raw)
        {
            var nodes = HidEnumerator.Enumerate(LogitechVid, 0);
            int pre = Precheck(nodes);
            if (pre != 0) return pre;

            Console.WriteLine("기기 찾는 중…");
            var all = new DeviceScanner(raw).Scan();
            SaveCache(Switchable(all));
            if (all.Count == 0)
            {
                Console.WriteLine("응답한 기기 없음 — 마우스·키보드가 지금 이 PC 에 연결돼 있는지 확인");
                return 1;
            }
            foreach (var r in all)
            {
                string ch = r.ChangeHostSupported
                    ? (r.HostCount > 0
                        ? string.Format("채널 {0}개 · 지금 {1}(이 PC)", r.HostCount, r.CurrentHost >= 0 ? (r.CurrentHost + 1).ToString(CultureInfo.InvariantCulture) : "?")
                        : "채널 정보 없음")
                    : "Easy-Switch 미지원";
                Console.WriteLine(string.Format("  {0} — {1} · {2} · {3}", r.DeviceName, r.ConnectionType, ch, r.Path));
            }
            return 0;
        }

        /// <summary>HID++ 노드가 있는지·열 권한이 있는지 먼저 본다(없으면 안내하고 끝).</summary>
        private static int Precheck(List<HidInterfaceInfo> nodes)
        {
            bool any = false, denied = false;
            foreach (var info in nodes)
            {
                if (info.OutputReportByteLength < 7) continue;
                any = true;
                try { using (HidStream.Open(info)) { } }
                catch (UnauthorizedAccessException) { denied = true; }
                catch { }
            }
            if (!any)
            {
                Console.Error.WriteLine("로지텍 HID++ 장치 없음 — 리시버(Bolt/Unifying)나 블루투스로 이 PC 에 연결돼 있어야 함");
                return 1;
            }
            if (denied)
            {
                Console.Error.WriteLine("장치 권한 없음 — 한 번만: sudo ./install.sh (udev 규칙 설치) 후 다시 실행. 급하면 sudo logiflow …");
                return 2;
            }
            return 0;
        }

        private static List<ScanResult> Switchable(List<ScanResult> all)
        {
            var list = new List<ScanResult>();
            foreach (var r in all) if (r.ChangeHostSupported && r.HostCount > 0) list.Add(r);
            return list;
        }

        private static List<ScanResult> ScanAndSave(DiagLogger raw)
        {
            var list = Switchable(new DeviceScanner(raw).Scan());
            if (list.Count > 0) SaveCache(list);
            return list;
        }

        // 캐시: 노드 정체(HID_PHYS|HID_ID) · deviceIndex · 0x1814 인덱스 · 채널 수 · 이름 (탭 구분).
        private static string CacheFile() { return Path.Combine(CacheDir(), "devices.tsv"); }

        private static string CacheDir()
        {
            string b = Environment.GetEnvironmentVariable("XDG_CACHE_HOME");
            if (string.IsNullOrEmpty(b)) b = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".cache");
            return Path.Combine(b, "logiflow");
        }

        private static void SaveCache(List<ScanResult> list)
        {
            try
            {
                Directory.CreateDirectory(CacheDir());
                var lines = new List<string>();
                foreach (var r in list)
                {
                    string id = HidEnumerator.IdentityOf(r.Path);
                    if (string.IsNullOrEmpty(id)) continue;
                    lines.Add(string.Join("\t", id, r.DeviceIndex.ToString(CultureInfo.InvariantCulture),
                        r.ChangeHostIndex.ToString(CultureInfo.InvariantCulture), r.HostCount.ToString(CultureInfo.InvariantCulture),
                        (r.DeviceName ?? "").Replace('\t', ' ')));
                }
                File.WriteAllLines(CacheFile(), lines);
            }
            catch
            {
            }
        }

        private static List<ScanResult> LoadCache(List<HidInterfaceInfo> nodes)
        {
            var list = new List<ScanResult>();
            try
            {
                if (!File.Exists(CacheFile())) return list;
                var byId = new Dictionary<string, HidInterfaceInfo>(StringComparer.Ordinal);
                foreach (var info in nodes)
                {
                    string id = HidEnumerator.IdentityOf(info.Path);
                    if (!string.IsNullOrEmpty(id)) byId[id] = info;
                }
                foreach (string line in File.ReadAllLines(CacheFile()))
                {
                    var f = line.Split('\t');
                    if (f.Length < 5) continue;
                    HidInterfaceInfo info;
                    byte dev, chIdx;
                    int hosts;
                    if (!byId.TryGetValue(f[0], out info)) continue;
                    if (!byte.TryParse(f[1], out dev) || !byte.TryParse(f[2], out chIdx) || !int.TryParse(f[3], out hosts)) continue;
                    list.Add(new ScanResult
                    {
                        Interface = info,
                        Path = info.Path,
                        VidPid = info.VidPid,
                        DeviceIndex = dev,
                        ChangeHostSupported = chIdx != 0,
                        ChangeHostIndex = chIdx,
                        HostCount = hosts,
                        DeviceName = f[4],
                        ConnectionType = dev == 0xFF ? "직접(USB/BT)" : string.Format("리시버 뒤(index {0})", dev),
                    });
                }
            }
            catch
            {
            }
            return list;
        }

        private static string Version()
        {
            var v = typeof(Program).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>();
            string s = v != null ? v.InformationalVersion : "0";
            int plus = s.IndexOf('+');
            return plus > 0 ? s.Substring(0, plus) : s;
        }

        /// <summary>CI 확인용 — 디스크립터 파서가 리시버·블루투스·일반 마우스를 맞게 가르는지.</summary>
        private static int SelfTest()
        {
            int fail = 0;
            Action<string, byte[], ushort, ushort, int> check = (label, desc, page, usage, len) =>
            {
                var info = new HidInterfaceInfo();
                HidEnumerator.ParseDescriptor(desc, info);
                bool ok = info.UsagePage == page && info.Usage == usage && info.OutputReportByteLength == len;
                Console.WriteLine(string.Format("{0}: page=0x{1:X4} usage=0x{2:X4} out={3} → {4}",
                    label, info.UsagePage, info.Usage, info.OutputReportByteLength, ok ? "OK" : "FAIL"));
                if (!ok) fail++;
            };
            // 리시버 HID++ 인터페이스(short 0x10 + long 0x11 + DJ 0x20/0x21).
            check("receiver", Hex("06 00 FF 09 01 A1 01 85 10 75 08 95 06 15 00 26 FF 00 09 01 81 00 09 01 91 00 C0 " +
                                  "06 00 FF 09 02 A1 01 85 11 75 08 95 13 15 00 26 FF 00 09 02 81 00 09 02 91 00 C0 " +
                                  "06 00 FF 09 04 A1 01 85 20 75 08 95 0E 15 00 26 FF 00 09 41 81 00 09 41 91 00 " +
                                  "85 21 95 1F 15 00 26 FF 00 09 42 81 00 09 42 91 00 C0"), 0xFF00, 0x0202, 20);
            // 블루투스 직접 연결(long 만, page 0xFF43).
            check("bluetooth", Hex("05 01 09 02 A1 01 85 02 09 01 A1 00 05 09 19 01 29 10 15 00 25 01 95 10 75 01 81 02 C0 C0 " +
                                   "06 43 FF 0A 02 02 A1 01 85 11 75 08 95 13 15 00 26 FF 00 09 02 81 00 09 02 91 00 C0"), 0xFF43, 0x0202, 20);
            // 일반 마우스(벤더 페이지 없음) — 후보 아님.
            check("mouse", Hex("05 01 09 02 A1 01 85 10 09 01 A1 00 05 09 19 01 29 05 15 00 25 01 95 05 75 01 81 02 C0 C0"), 0x0000, 0x0000, 0);
            // 이 머신 열거가 예외 없이 끝나는지.
            try { Console.WriteLine("enumerate: 로지텍 노드 " + HidEnumerator.Enumerate(LogitechVid, 0).Count + "개 → OK"); }
            catch (Exception ex) { Console.WriteLine("enumerate: " + ex.Message + " → FAIL"); fail++; }
            Console.WriteLine(fail == 0 ? "selftest OK" : "selftest FAIL " + fail);
            return fail == 0 ? 0 : 1;
        }

        private static byte[] Hex(string s)
        {
            var parts = s.Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);
            var b = new byte[parts.Length];
            for (int i = 0; i < parts.Length; i++) b[i] = byte.Parse(parts[i], NumberStyles.HexNumber, CultureInfo.InvariantCulture);
            return b;
        }
    }
}
