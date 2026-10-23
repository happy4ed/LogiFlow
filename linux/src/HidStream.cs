using System;
using System.Runtime.InteropServices;

namespace LogiFlow.Hid
{
    /// <summary>
    /// 리눅스 hidraw 노드 스트림 — 윈도우 HidStream 과 같은 API 라 HidppDevice 가 그대로 쓴다.
    /// 쓰기 = 리포트 ID 를 첫 바이트로 둔 리포트를 write(2) 로 그대로. 읽기 = poll(2) 로 기다린 뒤 read(2).
    /// 번호 붙은 리포트라 read 결과도 첫 바이트가 리포트 ID(윈도우 ReadFile 과 같은 모양).
    /// </summary>
    internal sealed class HidStream : IDisposable
    {
        private int _fd = -1;

        public int InputReportByteLength { get; private set; }
        public int OutputReportByteLength { get; private set; }

        private HidStream()
        {
        }

        public static HidStream Open(HidInterfaceInfo info)
        {
            int fd = Libc.open(info.Path, Libc.O_RDWR | Libc.O_NONBLOCK | Libc.O_CLOEXEC);
            if (fd < 0)
            {
                int err = Marshal.GetLastWin32Error();
                if (err == Libc.EACCES || err == Libc.EPERM)
                {
                    throw new UnauthorizedAccessException(info.Path + " 권한 없음");
                }
                throw new InvalidOperationException(string.Format("{0} 열기 실패(errno {1})", info.Path, err));
            }
            return new HidStream
            {
                _fd = fd,
                InputReportByteLength = info.InputReportByteLength,
                OutputReportByteLength = info.OutputReportByteLength,
            };
        }

        public bool Write(byte[] report, int timeoutMs)
        {
            if (_fd < 0 || report == null || report.Length == 0) return false;
            int deadline = Environment.TickCount + Math.Max(1, timeoutMs);
            while (true)
            {
                long n = (long)Libc.write(_fd, report, (UIntPtr)report.Length);
                if (n == report.Length) return true;
                int err = Marshal.GetLastWin32Error();
                if (n >= 0 || (err != Libc.EAGAIN && err != Libc.EINTR)) return false;
                int remaining = deadline - Environment.TickCount;
                if (remaining <= 0 || !Libc.PollOne(_fd, Libc.POLLOUT, remaining)) return false;
            }
        }

        public byte[] Read(int timeoutMs)
        {
            if (_fd < 0) return null;
            var buf = new byte[64];
            int deadline = Environment.TickCount + Math.Max(0, timeoutMs);
            while (true)
            {
                long n = (long)Libc.read(_fd, buf, (UIntPtr)buf.Length);
                if (n > 0)
                {
                    var r = new byte[n];
                    Array.Copy(buf, r, (int)n);
                    return r;
                }
                if (n < 0)
                {
                    int err = Marshal.GetLastWin32Error();
                    if (err != Libc.EAGAIN && err != Libc.EINTR) return null;
                }
                int remaining = deadline - Environment.TickCount;
                if (remaining <= 0 || !Libc.PollOne(_fd, Libc.POLLIN, remaining)) return null;
            }
        }

        public void Dispose()
        {
            if (_fd >= 0)
            {
                Libc.close(_fd);
                _fd = -1;
            }
        }
    }
}
