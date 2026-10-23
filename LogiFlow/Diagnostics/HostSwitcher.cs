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

        /// <summary>
        /// 로그온 워커 전용 — 각 기기 전환을 '보이는 로그'(say)에 기기별로 남기고, 전환이 확인될 때까지
        /// 짧게 재시도한다(콜드부팅 로그인 전엔 HID++ 명령 파이프가 덜 깨어 첫 쓰기가 무시될 수 있음).
        /// 쓰기 경로는 기존 HidppDevice.SetHost 하나뿐이며 안전 가드(지원·범위)는 SwitchOne 과 동일하게 적용한다.
        /// 전환 성공 판정: setHost 쓰기 성공 + (전환되어 응답 사라짐 || getHostInfo 현재채널==목표).
        /// </summary>
        public static int SwitchAllVerbose(IEnumerable<ScanResult> devices, int targetHost, DiagLogger raw, Action<string> say, int attempts)
        {
            int switched = 0;
            foreach (var d in devices)
            {
                if (d == null || d.Interface == null) continue;
                // (a) 0x1814 지원 (b) 채널 범위 — SwitchOne 과 동일 가드.
                if (!d.ChangeHostSupported || d.ChangeHostIndex == 0)
                {
                    say(string.Format("  {0}: 0x1814 미지원 — 건너뜀", d.DeviceName));
                    continue;
                }
                if (targetHost < 0 || (d.HostCount >= 0 && targetHost >= d.HostCount))
                {
                    say(string.Format("  {0}: 채널 {1} 범위 밖(호스트 {2}) — 건너뜀", d.DeviceName, targetHost + 1, d.HostCount));
                    continue;
                }

                bool ok = false;
                for (int a = 1; a <= attempts && !ok; a++)
                {
                    HidStream stream = null;
                    try
                    {
                        stream = HidStream.Open(d.Interface);
                        var hidpp = new HidppDevice(stream, raw);
                        // 명령 파이프 깨우기(ping) — 콜드부팅 직후 첫 명령이 무시되는 것 완화.
                        try { hidpp.Ping(d.DeviceIndex, 200); } catch { }
                        bool wrote = hidpp.SetHost(d.DeviceIndex, d.ChangeHostIndex, (byte)targetHost, WriteTimeoutMs);
                        int nb, cur; bool got = hidpp.GetHostInfo(d.DeviceIndex, d.ChangeHostIndex, 300, out nb, out cur);
                        // 전환되면 기기가 이 리시버에서 사라져 getHostInfo 응답이 없는 게 정상(= 성공 신호).
                        if (wrote && (!got || cur == targetHost)) ok = true;
                        say(string.Format("  {0}: 시도{1} 쓰기={2} 확인={3} → {4}",
                            d.DeviceName, a, wrote ? "성공" : "실패",
                            got ? ("현재채널 " + (cur + 1)) : "응답없음(전환추정)",
                            ok ? "전환됨" : "미전환"));
                    }
                    catch (Exception ex) { say(string.Format("  {0}: 시도{1} 예외 {2}", d.DeviceName, a, ex.Message)); }
                    finally { if (stream != null) stream.Dispose(); }
                    if (!ok) System.Threading.Thread.Sleep(150);
                }
                if (ok) switched++;
            }
            return switched;
        }

        /// <summary>
        /// 바탕화면 워커 전용 — 모든 기기에 setHost 를 기기 사이 대기 없이 '먼저 연달아' 쓰고, 그다음 확인한다.
        /// (기존 SwitchAllVerbose 는 기기마다 ping 200ms + 확인 300ms 를 기다린 뒤 다음 기기로 가서 두 번째 기기가
        /// 0.5초 이상 늦게 넘어갔다.) 첫 쓰기가 성공한 순간 onDelivered 를 부른다 — glow 최고점 = 실제 전달 시점.
        /// 첫 시도는 깨우기(ping) 없이 바로 쓴다(바탕화면에서 마우스를 밀고 있으면 기기는 깨어 있음).
        /// 실패한 기기만 ping 으로 깨운 뒤 재시도. 쓰기 경로(HidppDevice.SetHost)·가드는 SwitchAllVerbose 와 동일.
        /// </summary>
        public static int SwitchAllBurst(IEnumerable<ScanResult> devices, int targetHost, DiagLogger raw, Action<string> say, int attempts, Action onDelivered)
        {
            var pending = new List<ScanResult>();
            foreach (var d in devices)
            {
                if (d == null || d.Interface == null) continue;
                // (a) 0x1814 지원 (b) 채널 범위 — SwitchOne 과 동일 가드.
                if (!d.ChangeHostSupported || d.ChangeHostIndex == 0)
                {
                    say(string.Format("  {0}: 0x1814 미지원 — 건너뜀", d.DeviceName));
                    continue;
                }
                if (targetHost < 0 || (d.HostCount >= 0 && targetHost >= d.HostCount))
                {
                    say(string.Format("  {0}: 채널 {1} 범위 밖(호스트 {2}) — 건너뜀", d.DeviceName, targetHost + 1, d.HostCount));
                    continue;
                }
                pending.Add(d);
            }

            int switched = 0;
            bool delivered = false;
            for (int a = 1; a <= attempts && pending.Count > 0; a++)
            {
                var streams = new HidStream[pending.Count];
                var hidpps = new HidppDevice[pending.Count];
                var wrote = new bool[pending.Count];
                try
                {
                    // 1) 쓰기 묶음.
                    for (int i = 0; i < pending.Count; i++)
                    {
                        var d = pending[i];
                        try
                        {
                            streams[i] = HidStream.Open(d.Interface);
                            hidpps[i] = new HidppDevice(streams[i], raw);
                            if (a > 1) { try { hidpps[i].Ping(d.DeviceIndex, 200); } catch { } }
                            wrote[i] = hidpps[i].SetHost(d.DeviceIndex, d.ChangeHostIndex, (byte)targetHost, WriteTimeoutMs, 0);
                        }
                        catch (Exception ex) { say(string.Format("  {0}: 시도{1} 쓰기 예외 {2}", d.DeviceName, a, ex.Message)); }
                        if (wrote[i] && !delivered)
                        {
                            delivered = true;
                            if (onDelivered != null) { try { onDelivered(); } catch { } }
                        }
                    }

                    // 2) 확인 — 전환되면 기기가 이 리시버에서 사라져 응답이 없는 게 정상(= 성공 신호).
                    var next = new List<ScanResult>();
                    for (int i = 0; i < pending.Count; i++)
                    {
                        var d = pending[i];
                        bool ok = false, got = false;
                        int nb, cur = -1;
                        string why = null;
                        if (wrote[i])
                        {
                            try { got = hidpps[i].GetHostInfo(d.DeviceIndex, d.ChangeHostIndex, 300, out nb, out cur); ok = !got || cur == targetHost; }
                            catch (Exception ex) { why = "확인 예외 " + ex.Message; }
                        }
                        say(string.Format("  {0}: 시도{1} 쓰기={2} 확인={3} → {4}",
                            d.DeviceName, a, wrote[i] ? "성공" : "실패",
                            why ?? (!wrote[i] ? "-" : got ? ("현재채널 " + (cur + 1)) : "응답없음(전환추정)"),
                            ok ? "전환됨" : "미전환"));
                        if (ok) switched++; else next.Add(d);
                    }
                    pending = next;
                }
                finally
                {
                    foreach (var st in streams) if (st != null) { try { st.Dispose(); } catch { } }
                }
                if (pending.Count > 0 && a < attempts) System.Threading.Thread.Sleep(150);
            }
            return switched;
        }
    }
}
