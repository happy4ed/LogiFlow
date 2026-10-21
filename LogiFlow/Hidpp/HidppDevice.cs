using System;
using System.Text;
using LogiFlow.Diagnostics;
using LogiFlow.Hid;

namespace LogiFlow.Hidpp
{
    /// <summary>
    /// 한 HID++ 인터페이스(long 0x0202 또는 short 0x0201) 위에서 특정 deviceIndex 를 상대로
    /// 조회성 호출을 수행한다. 출력 리포트 길이로 long(0x11)/short(0x10) 프레임을 자동 선택한다.
    /// 읽기 전용: func0 / IRoot(0x0000) / DeviceName(0x0005) 만 보낸다.
    /// </summary>
    internal sealed class HidppDevice
    {
        public const byte ReportIdLong = 0x11;
        public const byte ReportIdShort = 0x10;

        // 응답 매칭용 software id (0 아닌 4비트 값).
        private const byte SoftwareId = 0x0A;

        // 기능/인덱스 상수.
        public const byte IRootIndex = 0x00;
        public const ushort FeatureChangeHost = 0x1814;
        public const ushort FeatureDeviceName = 0x0005;

        private readonly HidStream _stream;
        private readonly DiagLogger _log;

        public HidppDevice(HidStream stream, DiagLogger log)
        {
            _stream = stream;
            _log = log;
        }

        private static byte FuncByte(byte funcIndex)
        {
            return (byte)(((funcIndex & 0x0F) << 4) | SoftwareId);
        }

        /// <summary>
        /// HID++ 리포트로 호출하고 응답 payload(파라미터)를 돌려준다.
        /// 인터페이스가 long(20바이트)이면 0x11, 아니면 short(0x10·7바이트) 프레임을 쓴다.
        /// 타임아웃 시 null, 오류 응답 시 HidppErrorException.
        /// </summary>
        public byte[] Call(byte deviceIndex, byte featureIndex, byte funcIndex, byte[] parms, int timeoutMs)
        {
            byte funcByte = FuncByte(funcIndex);

            // 출력 리포트 길이로 프레임 종류 결정: >=20 이면 long(0x11), 아니면 short(0x10).
            bool useLong = _stream.OutputReportByteLength >= 20;
            int reportLen = useLong ? 20 : 7;
            int maxParams = useLong ? 16 : 3;

            var report = new byte[reportLen];
            report[0] = useLong ? ReportIdLong : ReportIdShort;
            report[1] = deviceIndex;
            report[2] = featureIndex;
            report[3] = funcByte;
            if (parms != null)
            {
                int n = Math.Min(parms.Length, maxParams);
                Array.Copy(parms, 0, report, 4, n);
            }

            _log.LogTx(deviceIndex, report);

            if (!_stream.Write(report, timeoutMs))
            {
                _log.LogTimeout(string.Format("Write dev=0x{0:X2} feat=0x{1:X2} func={2}", deviceIndex, featureIndex, funcIndex));
                return null;
            }

            int deadline = Environment.TickCount + timeoutMs;
            while (true)
            {
                int remaining = deadline - Environment.TickCount;
                if (remaining <= 0)
                {
                    _log.LogTimeout(string.Format("Read dev=0x{0:X2} feat=0x{1:X2} func={2}", deviceIndex, featureIndex, funcIndex));
                    return null;
                }

                byte[] resp = _stream.Read(remaining);
                if (resp == null)
                {
                    _log.LogTimeout(string.Format("Read dev=0x{0:X2} feat=0x{1:X2} func={2}", deviceIndex, featureIndex, funcIndex));
                    return null;
                }

                _log.LogRx(resp);

                if (resp.Length < 4)
                {
                    continue;
                }

                byte rId = resp[0];
                byte rDev = resp[1];
                byte rFeat = resp[2];
                byte rFunc = resp[3];

                // HID++ 2.0 오류 응답: feature index 자리에 0xFF.
                if ((rId == ReportIdLong || rId == ReportIdShort) && rFeat == 0xFF
                    && rDev == deviceIndex && resp.Length >= 6
                    && resp[3] == featureIndex && resp[4] == funcByte)
                {
                    int code = resp[5];
                    throw new HidppErrorException(code,
                        "HID++2.0 " + HidppErrorException.Describe(code));
                }

                // HID++ 1.0 오류 응답(리시버): 0x8F.
                if (rId == ReportIdShort && rFeat == 0x8F && rDev == deviceIndex)
                {
                    int code = resp.Length >= 6 ? resp[5] : -1;
                    throw new HidppErrorException(code, "HID++1.0 error 0x" + code.ToString("X2"));
                }

                // 정상 응답 매칭.
                if (rDev == deviceIndex && rFeat == featureIndex && rFunc == funcByte)
                {
                    var payload = new byte[16];
                    int copy = Math.Min(16, resp.Length - 4);
                    if (copy > 0)
                    {
                        Array.Copy(resp, 4, payload, 0, copy);
                    }
                    return payload;
                }

                // 그 밖은 비동기 알림 — 무시하고 계속 읽는다.
            }
        }

        /// <summary>IRoot ping(func1): 응답 echo 로 장치 존재 확인.</summary>
        public bool Ping(byte deviceIndex, int timeoutMs)
        {
            const byte marker = 0x5A;
            byte[] payload;
            try
            {
                payload = Call(deviceIndex, IRootIndex, 1, new byte[] { 0x00, 0x00, marker }, timeoutMs);
            }
            catch (HidppErrorException)
            {
                // 오류 응답도 "장치가 응답했다"는 증거다.
                return true;
            }
            if (payload == null)
            {
                return false;
            }
            return payload[2] == marker;
        }

        /// <summary>IRoot getFeature(func0): featureId 의 런타임 인덱스. 0 이면 미지원.</summary>
        public byte GetFeatureIndex(byte deviceIndex, ushort featureId, int timeoutMs)
        {
            byte[] parms = new byte[] { (byte)(featureId >> 8), (byte)(featureId & 0xFF) };
            byte[] payload = Call(deviceIndex, IRootIndex, 0, parms, timeoutMs);
            if (payload == null)
            {
                return 0;
            }
            return payload[0];
        }

        /// <summary>0x1814 func0 getHostInfo → (호스트 개수, 현재 호스트 0-base).</summary>
        public bool GetHostInfo(byte deviceIndex, byte changeHostIndex, int timeoutMs, out int nbHost, out int currHost)
        {
            nbHost = -1;
            currHost = -1;
            byte[] payload = Call(deviceIndex, changeHostIndex, 0, null, timeoutMs);
            if (payload == null)
            {
                return false;
            }
            nbHost = payload[0];
            currHost = payload[1];
            return true;
        }

        /// <summary>0x0005 로 기기 이름을 읽는다(best-effort). 실패 시 null.</summary>
        public string GetDeviceName(byte deviceIndex, byte nameFeatureIndex, int timeoutMs)
        {
            try
            {
                byte[] head = Call(deviceIndex, nameFeatureIndex, 0, null, timeoutMs); // getCount
                if (head == null)
                {
                    return null;
                }
                int count = head[0];
                if (count <= 0 || count > 64)
                {
                    return null;
                }

                var sb = new StringBuilder(count);
                int idx = 0;
                int guard = 0;
                while (idx < count && guard < 16)
                {
                    guard++;
                    byte[] chunk = Call(deviceIndex, nameFeatureIndex, 1, new byte[] { (byte)idx }, timeoutMs); // getName(idx)
                    if (chunk == null)
                    {
                        break;
                    }
                    int added = 0;
                    for (int i = 0; i < chunk.Length && idx < count; i++)
                    {
                        byte b = chunk[i];
                        if (b == 0)
                        {
                            break;
                        }
                        if (b >= 0x20 && b < 0x7F)
                        {
                            sb.Append((char)b);
                        }
                        idx++;
                        added++;
                    }
                    if (added == 0)
                    {
                        break;
                    }
                }
                string name = sb.ToString().Trim();
                return name.Length > 0 ? name : null;
            }
            catch (HidppErrorException)
            {
                return null;
            }
        }
    }
}
