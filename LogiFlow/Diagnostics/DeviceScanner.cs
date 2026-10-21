using System;
using System.Collections.Generic;
using LogiFlow.Hid;
using LogiFlow.Hidpp;

namespace LogiFlow.Diagnostics
{
    /// <summary>
    /// 로지텍 HID++ 장치를 열거하고, 각 후보에 대해 런타임으로
    /// 0x1814 인덱스·호스트 정보·기기 이름을 조회한다. 읽기 전용.
    /// </summary>
    internal sealed class DeviceScanner
    {
        public const ushort LogitechVid = 0x046D;
        public const ushort HidppUsagePage = 0xFF43;
        public const ushort UsageLong = 0x0202;
        public const ushort UsageShort = 0x0201;

        // deviceIndex 후보: 직접 연결(0xFF) + 리시버 뒤 1~6.
        private static readonly byte[] DeviceIndexCandidates =
            new byte[] { 0xFF, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06 };

        private const int ProbeTimeoutMs = 600;
        private const int QueryTimeoutMs = 800;

        private readonly DiagLogger _log;

        public DeviceScanner(DiagLogger log)
        {
            _log = log;
        }

        public List<ScanResult> Scan()
        {
            var results = new List<ScanResult>();

            _log.Log("=== 스캔 시작 ===");
            List<HidInterfaceInfo> interfaces;
            try
            {
                // usagePage 필터 없이 VID 046D 인터페이스를 전부 가져와 진단에 남긴다.
            interfaces = HidEnumerator.Enumerate(LogitechVid, 0);
            }
            catch (Exception ex)
            {
                _log.LogError("열거 실패: " + ex.Message);
                return results;
            }

            _log.Log(string.Format("로지텍(VID 046D) HID 인터페이스 {0}개 발견", interfaces.Count));
            foreach (var iface in interfaces)
            {
                _log.Log(string.Format("  iface {0} usagePage=0x{1:X4} usage=0x{2:X4} in={3} out={4} caps={5} path={6}",
                    iface.VidPid, iface.UsagePage, iface.Usage, iface.InputReportByteLength,
                    iface.OutputReportByteLength, iface.CapsRead ? "ok" : "unread", iface.Path));
            }

            foreach (var iface in interfaces)
            {
                // HID++ 후보: long(usage 0x0202, 20바이트) + short(usage 0x0201, 7바이트)
                // 벤더 컬렉션, 그리고 벤더 usagePage(0xFFxx)에 7바이트 이상 출력 리포트가 있는
                // 인터페이스 모두. caps 를 못 읽은 것(다른 프로세스 선점)은 건너뛰되 로그엔 남는다.
                bool candidate = iface.CapsRead &&
                    (iface.Usage == UsageLong || iface.Usage == UsageShort ||
                     (iface.UsagePage >= 0xFF00 && iface.OutputReportByteLength >= 7));
                _log.Log(string.Format("  iface {0} usage=0x{1:X4} 후보={2}",
                    iface.VidPid, iface.Usage, candidate ? "예" : "아니오"));
                if (!candidate)
                {
                    continue;
                }

                ProbeInterface(iface, results);
            }

            List<ScanResult> deduped = Deduplicate(results);

            if (deduped.Count == 0)
            {
                _log.Log("응답한 HID++ 장치 없음");
            }
            _log.Log(string.Format("=== 스캔 종료: 후보 응답 {0}건 → 중복 제거 후 장치 {1}개 ===",
                results.Count, deduped.Count));
            return deduped;
        }

        /// <summary>
        /// 같은 물리 기기가 여러 인터페이스(long/short)로 잡히면 하나로 합친다.
        /// 키 = 이름 + deviceIndex + 연결 방식. 서로 다른 기기는 모두 남긴다.
        /// 정보가 더 풍부한 쪽(0x1814 지원)을 유지한다.
        /// </summary>
        private static List<ScanResult> Deduplicate(List<ScanResult> results)
        {
            var map = new Dictionary<string, ScanResult>();
            var order = new List<string>();
            foreach (var r in results)
            {
                string key = (r.DeviceName ?? "") + "|" + r.DeviceIndex + "|" + (r.ConnectionType ?? "");
                ScanResult existing;
                if (!map.TryGetValue(key, out existing))
                {
                    map[key] = r;
                    order.Add(key);
                }
                else if (!existing.ChangeHostSupported && r.ChangeHostSupported)
                {
                    // 0x1814 정보를 더 많이 가진 쪽으로 교체.
                    map[key] = r;
                }
            }

            var list = new List<ScanResult>();
            foreach (var key in order)
            {
                list.Add(map[key]);
            }
            return list;
        }

        private void ProbeInterface(HidInterfaceInfo iface, List<ScanResult> results)
        {
            HidStream stream = null;
            try
            {
                stream = HidStream.Open(iface);
            }
            catch (Exception ex)
            {
                _log.LogError(string.Format("인터페이스 열기 실패 {0}: {1}", iface.Path, ex.Message));
                return;
            }

            try
            {
                var hidpp = new HidppDevice(stream, _log);

                foreach (byte deviceIndex in DeviceIndexCandidates)
                {
                    bool present;
                    try
                    {
                        // 잠자는 기기(특히 키보드)를 고려해 1회 재시도.
                        present = hidpp.Ping(deviceIndex, ProbeTimeoutMs)
                                  || hidpp.Ping(deviceIndex, ProbeTimeoutMs);
                    }
                    catch (Exception ex)
                    {
                        _log.LogError(string.Format("ping dev=0x{0:X2}: {1}", deviceIndex, ex.Message));
                        present = false;
                    }

                    // 각 인터페이스×deviceIndex 핑 결과를 한 줄씩 남긴다(안 잡힌 index 가 타임아웃인지 가시화).
                    _log.Log(string.Format("  ping {0} dev=0x{1:X2} → {2}",
                        iface.VidPid, deviceIndex, present ? "응답" : "타임아웃"));

                    if (!present)
                    {
                        continue;
                    }

                    results.Add(BuildResult(iface, hidpp, deviceIndex));
                }
            }
            finally
            {
                stream.Dispose();
            }
        }

        private ScanResult BuildResult(HidInterfaceInfo iface, HidppDevice hidpp, byte deviceIndex)
        {
            var r = new ScanResult
            {
                Interface = iface,
                VidPid = iface.VidPid,
                Path = iface.Path,
                DeviceIndex = deviceIndex,
                ConnectionType = deviceIndex == 0xFF
                    ? "직접(USB/BT)"
                    : string.Format("리시버 뒤(index {0})", deviceIndex),
            };

            // 0x1814 Change Host 인덱스 런타임 조회.
            try
            {
                byte chIndex = hidpp.GetFeatureIndex(deviceIndex, HidppDevice.FeatureChangeHost, QueryTimeoutMs);
                if (chIndex != 0)
                {
                    r.ChangeHostSupported = true;
                    r.ChangeHostIndex = chIndex;
                    int nb, cur;
                    if (hidpp.GetHostInfo(deviceIndex, chIndex, QueryTimeoutMs, out nb, out cur))
                    {
                        r.HostCount = nb;
                        r.CurrentHost = cur;
                    }
                    else
                    {
                        r.Note = AppendNote(r.Note, "getHostInfo 무응답");
                    }
                }
                else
                {
                    r.ChangeHostSupported = false;
                    r.Note = AppendNote(r.Note, "0x1814 미지원");
                }
            }
            catch (Exception ex)
            {
                r.Note = AppendNote(r.Note, "0x1814 조회 오류: " + ex.Message);
            }

            // 기기 이름: HID++ 0x0005 우선, 실패 시 Windows 제품 문자열.
            try
            {
                byte nameIndex = hidpp.GetFeatureIndex(deviceIndex, HidppDevice.FeatureDeviceName, QueryTimeoutMs);
                if (nameIndex != 0)
                {
                    r.DeviceName = hidpp.GetDeviceName(deviceIndex, nameIndex, QueryTimeoutMs);
                }
            }
            catch (Exception ex)
            {
                _log.LogError(string.Format("0x0005 조회 오류 dev=0x{0:X2}: {1}", deviceIndex, ex.Message));
            }

            if (string.IsNullOrEmpty(r.DeviceName))
            {
                r.DeviceName = !string.IsNullOrEmpty(iface.ProductString)
                    ? iface.ProductString
                    : "(이름 미상)";
            }

            return r;
        }

        private static string AppendNote(string existing, string add)
        {
            if (string.IsNullOrEmpty(existing))
            {
                return add;
            }
            return existing + "; " + add;
        }
    }
}
