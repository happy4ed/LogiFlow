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
                interfaces = HidEnumerator.Enumerate(LogitechVid, HidppUsagePage);
            }
            catch (Exception ex)
            {
                _log.LogError("열거 실패: " + ex.Message);
                return results;
            }

            _log.Log(string.Format("로지텍 HID++ 인터페이스 {0}개 발견", interfaces.Count));
            foreach (var iface in interfaces)
            {
                _log.Log(string.Format("  iface {0} usage=0x{1:X4} in={2} out={3} path={4}",
                    iface.VidPid, iface.Usage, iface.InputReportByteLength, iface.OutputReportByteLength, iface.Path));
            }

            foreach (var iface in interfaces)
            {
                // HID++ long 컬렉션(0x0202)에서만 프레임을 주고받는다.
                if (iface.Usage != UsageLong)
                {
                    continue;
                }

                ProbeInterface(iface, results);
            }

            if (results.Count == 0)
            {
                _log.Log("응답한 HID++ 장치 없음");
            }
            _log.Log(string.Format("=== 스캔 종료: 장치 {0}개 ===", results.Count));
            return results;
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
                        present = hidpp.Ping(deviceIndex, ProbeTimeoutMs);
                    }
                    catch (Exception ex)
                    {
                        _log.LogError(string.Format("ping dev=0x{0:X2}: {1}", deviceIndex, ex.Message));
                        present = false;
                    }

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
