using System;
using System.Collections.Generic;
using LogiFlow.Hid;
using LogiFlow.Hidpp;

namespace LogiFlow.Diagnostics
{
    /// <summary>
    /// 통제된 쓰기: 0x1814 func1 setHost 로 호스트(채널)를 전환한다.
    /// 안전 가드 — (a)0x1814 지원 확인된 기기 (b)targetHost 가 0~hostCount-1 범위
    /// (c)명시적 전환 버튼에서만 호출. 이 클래스 밖에서는 어떤 쓰기도 하지 않는다.
    /// </summary>
    internal static class HostSwitcher
    {
        private const int WriteTimeoutMs = 400;

        /// <summary>단일 기기 전환. 가드 위반 시 false(아무것도 전송 안 함).</summary>
        public static bool SwitchOne(ScanResult device, int targetHost, DiagLogger log)
        {
            if (device == null || device.Interface == null)
            {
                return false;
            }
            // (a) 0x1814 지원 확인.
            if (!device.ChangeHostSupported || device.ChangeHostIndex == 0)
            {
                log.LogError(string.Format("전환 거부(0x1814 미지원): {0}", device.DeviceName));
                return false;
            }
            // (b) 채널 범위 검증.
            if (targetHost < 0 || (device.HostCount >= 0 && targetHost >= device.HostCount))
            {
                log.LogError(string.Format("전환 거부(범위 밖 채널 {0}, 호스트 {1}): {2}",
                    targetHost, device.HostCount, device.DeviceName));
                return false;
            }

            HidStream stream = null;
            try
            {
                stream = HidStream.Open(device.Interface);
                var hidpp = new HidppDevice(stream, log);
                log.Log(string.Format("전환 시도: {0} (dev=0x{1:X2}, 0x1814 index=0x{2:X2}) → 채널 {3}",
                    device.DeviceName, device.DeviceIndex, device.ChangeHostIndex, targetHost + 1));
                return hidpp.SetHost(device.DeviceIndex, device.ChangeHostIndex, (byte)targetHost, WriteTimeoutMs);
            }
            catch (Exception ex)
            {
                log.LogError(string.Format("전환 예외: {0}: {1}", device.DeviceName, ex.Message));
                return false;
            }
            finally
            {
                if (stream != null)
                {
                    stream.Dispose();
                }
            }
        }

        /// <summary>
        /// 전환 가능한 모든 기기(마우스·키보드 등)를 같은 채널로 함께 전환한다.
        /// 각 기기는 자기 feature index·device index 로 setHost 를 받는다.
        /// </summary>
        public static int SwitchAll(IEnumerable<ScanResult> devices, int targetHost, DiagLogger log)
        {
            int sent = 0;
            log.Log(string.Format("=== 전체 전환 시작 → 채널 {0} ===", targetHost + 1));
            foreach (var d in devices)
            {
                if (!d.ChangeHostSupported)
                {
                    continue;
                }
                if (SwitchOne(d, targetHost, log))
                {
                    sent++;
                }
            }
            log.Log(string.Format("=== 전체 전환 종료: {0}개 전송 ===", sent));
            return sent;
        }
    }
}
