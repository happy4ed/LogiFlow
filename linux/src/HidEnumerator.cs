using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text.RegularExpressions;

namespace LogiFlow.Hid
{
    /// <summary>
    /// 리눅스 hidraw 열거 — /sys/class/hidraw/* 를 읽어 윈도우 HidEnumerator 와 같은 HidInterfaceInfo 를 만든다.
    ///
    /// hid-logitech-dj(우분투 기본)는 리시버 뒤 기기마다 자식 HID 장치·hidraw 노드를 따로 만든다. 같은 기기에
    /// 두 경로로 setHost 가 가지 않게 자식 노드는 건너뛰고 리시버 노드로만 보낸다. 대신 자식의 HID_PHYS 끝
    /// ":N" 이 페어링된 기기 index 라, 리시버 노드는 그 index 만 핑하게 힌트를 준다(0xFF+1~6 전부 핑하면
    /// 없는 index 마다 타임아웃이라 수 초 걸림).
    /// </summary>
    internal static class HidEnumerator
    {
        private const string ClassRoot = "/sys/class/hidraw";
        private static readonly Regex HidDevName = new Regex(@"^[0-9A-Fa-f]{4}:[0-9A-Fa-f]{4}:[0-9A-Fa-f]{4}\.[0-9A-Fa-f]{4}$");

        public static List<HidInterfaceInfo> Enumerate(ushort vendorIdFilter, ushort usagePageFilter)
        {
            var list = new List<HidInterfaceInfo>();
            if (!Directory.Exists(ClassRoot)) return list;

            var nodes = new List<string>(Directory.GetDirectories(ClassRoot));
            nodes.Sort(StringComparer.Ordinal);
            foreach (string node in nodes)
            {
                string name = Path.GetFileName(node);
                string hidDir = Libc.RealPath(Path.Combine(node, "device"));
                if (hidDir == null) continue;

                var ue = ReadUevent(Path.Combine(hidDir, "uevent"));
                ushort bus, vid, pid;
                if (!TryParseHidId(Get(ue, "HID_ID"), out bus, out vid, out pid)) continue;
                if (vendorIdFilter != 0 && vid != vendorIdFilter) continue;

                // dj 자식(부모 디렉터리가 또 하나의 HID 장치)은 건너뛴다.
                string parent = Path.GetFileName(Path.GetDirectoryName(hidDir) ?? "");
                if (HidDevName.IsMatch(parent)) continue;

                var info = new HidInterfaceInfo
                {
                    Path = "/dev/" + name,
                    VendorId = vid,
                    ProductId = pid,
                    ProductString = Get(ue, "HID_NAME"),
                };
                try { ParseDescriptor(File.ReadAllBytes(Path.Combine(hidDir, "report_descriptor")), info); }
                catch { continue; }
                if (usagePageFilter != 0 && info.UsagePage != usagePageFilter) continue;

                info.DeviceIndexHint = IndexHint(hidDir);
                list.Add(info);
            }
            return list;
        }

        /// <summary>캐시 대조용 — /dev/hidrawN 번호는 재부팅·재연결마다 바뀌므로 HID_PHYS+HID_ID 로 노드를 알아본다.</summary>
        public static string IdentityOf(string devPath)
        {
            string hidDir = Libc.RealPath(Path.Combine(ClassRoot, Path.GetFileName(devPath), "device"));
            if (hidDir == null) return null;
            var ue = ReadUevent(Path.Combine(hidDir, "uevent"));
            return Get(ue, "HID_PHYS") + "|" + Get(ue, "HID_ID");
        }

        /// <summary>
        /// 핑할 deviceIndex 힌트. dj 자식이 있으면 그 index 들, 직접 연결 기기(logitech-hidpp-device)면 0xFF,
        /// 모르면 null(0xFF+1~6 전부).
        /// </summary>
        private static byte[] IndexHint(string hidDir)
        {
            var idx = new SortedSet<byte>();
            try
            {
                foreach (string child in Directory.GetDirectories(hidDir))
                {
                    if (!HidDevName.IsMatch(Path.GetFileName(child))) continue;
                    string phys = Get(ReadUevent(Path.Combine(child, "uevent")), "HID_PHYS");
                    int colon = phys.LastIndexOf(':');
                    int n;
                    if (colon >= 0 && int.TryParse(phys.Substring(colon + 1), NumberStyles.Integer, CultureInfo.InvariantCulture, out n)
                        && n >= 1 && n <= 6)
                    {
                        idx.Add((byte)n);
                    }
                }
            }
            catch
            {
            }
            if (idx.Count > 0) return new List<byte>(idx).ToArray();

            string driver = Path.GetFileName(Libc.RealPath(Path.Combine(hidDir, "driver")) ?? "");
            if (driver == "logitech-hidpp-device") return new byte[] { 0xFF };
            return null;
        }

        /// <summary>
        /// 리포트 디스크립터에서 HID++ 흔적만 읽는다: 벤더 usage page(0xFF00·0xFF43) 아래의
        /// Report ID 0x10(short·7바이트)·0x11(long·20바이트). long 이 있으면 HidppDevice 가 long 프레임을 쓴다.
        /// </summary>
        internal static void ParseDescriptor(byte[] d, HidInterfaceInfo info)
        {
            ushort page = 0, vendorPage = 0, vendorUsage = 0;
            bool hasShort = false, hasLong = false;
            int i = 0;
            while (i < d.Length)
            {
                byte b = d[i];
                if (b == 0xFE)
                {
                    // long item: FE size tag data...
                    if (i + 1 >= d.Length) break;
                    i += 3 + d[i + 1];
                    continue;
                }
                int size = b & 0x3;
                if (size == 3) size = 4;
                if (i + size >= d.Length) break;
                uint val = 0;
                for (int k = 0; k < size; k++) val |= (uint)d[i + 1 + k] << (8 * k);

                switch (b & 0xFC)
                {
                    case 0x04: // Usage Page (global)
                        page = (ushort)val;
                        if (page >= 0xFF00 && vendorPage == 0) vendorPage = page;
                        break;
                    case 0x08: // Usage (local)
                        if (page >= 0xFF00 && vendorUsage == 0) vendorUsage = (ushort)val;
                        break;
                    case 0x84: // Report ID (global)
                        if (page >= 0xFF00)
                        {
                            if (val == 0x10) hasShort = true;
                            if (val == 0x11) hasLong = true;
                        }
                        break;
                }
                i += 1 + size;
            }

            info.UsagePage = vendorPage;
            info.Usage = hasLong ? (ushort)0x0202 : hasShort ? (ushort)0x0201 : vendorUsage;
            int len = hasLong ? 20 : hasShort ? 7 : 0;
            info.InputReportByteLength = len;
            info.OutputReportByteLength = len;
            info.CapsRead = true;
        }

        private static bool TryParseHidId(string s, out ushort bus, out ushort vid, out ushort pid)
        {
            bus = vid = pid = 0;
            // HID_ID=0003:0000046D:0000C548
            var parts = (s ?? "").Split(':');
            if (parts.Length != 3) return false;
            uint b, v, p;
            if (!uint.TryParse(parts[0], NumberStyles.HexNumber, CultureInfo.InvariantCulture, out b)) return false;
            if (!uint.TryParse(parts[1], NumberStyles.HexNumber, CultureInfo.InvariantCulture, out v)) return false;
            if (!uint.TryParse(parts[2], NumberStyles.HexNumber, CultureInfo.InvariantCulture, out p)) return false;
            bus = (ushort)b; vid = (ushort)v; pid = (ushort)p;
            return true;
        }

        private static Dictionary<string, string> ReadUevent(string path)
        {
            var map = new Dictionary<string, string>(StringComparer.Ordinal);
            try
            {
                foreach (string line in File.ReadAllLines(path))
                {
                    int eq = line.IndexOf('=');
                    if (eq > 0) map[line.Substring(0, eq)] = line.Substring(eq + 1);
                }
            }
            catch
            {
            }
            return map;
        }

        private static string Get(Dictionary<string, string> map, string key)
        {
            string v;
            return map.TryGetValue(key, out v) ? v : "";
        }
    }
}
