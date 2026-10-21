using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
using LogiFlow.Native;

namespace LogiFlow.Hid
{
    /// <summary>
    /// HID 인터페이스 1개에 대한 오버랩드 읽기/쓰기(타임아웃 지원).
    /// 읽기 전용 진단용이지만, HID++ 요청 송신에는 출력 리포트 WriteFile 이 필요하다
    /// (전송하는 명령은 조회성 func0/0x0000/0x0005 뿐 — 쓰기 명령은 보내지 않는다).
    /// </summary>
    internal sealed class HidStream : IDisposable
    {
        private readonly SafeFileHandle _handle;

        public int InputReportByteLength { get; private set; }
        public int OutputReportByteLength { get; private set; }

        private HidStream(SafeFileHandle handle, int inputLen, int outputLen)
        {
            _handle = handle;
            InputReportByteLength = inputLen;
            OutputReportByteLength = outputLen;
        }

        public static HidStream Open(HidInterfaceInfo info)
        {
            SafeFileHandle handle = NativeMethods.CreateFile(
                info.Path,
                NativeMethods.GENERIC_READ | NativeMethods.GENERIC_WRITE,
                NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE,
                IntPtr.Zero,
                NativeMethods.OPEN_EXISTING,
                NativeMethods.FILE_FLAG_OVERLAPPED,
                IntPtr.Zero);

            if (handle.IsInvalid)
            {
                int err = Marshal.GetLastWin32Error();
                handle.Dispose();
                throw new InvalidOperationException(
                    string.Format("HID 인터페이스 열기 실패 (Win32 오류 {0})", err));
            }

            return new HidStream(handle, info.InputReportByteLength, info.OutputReportByteLength);
        }

        /// <summary>출력 리포트 전송. report[0] 은 리포트 ID.</summary>
        public bool Write(byte[] report, int timeoutMs)
        {
            int len = OutputReportByteLength > 0 ? OutputReportByteLength : report.Length;
            var buffer = new byte[len];
            Array.Copy(report, buffer, Math.Min(report.Length, len));

            var ov = new NativeMethods.OVERLAPPED();
            ov.hEvent = NativeMethods.CreateEvent(IntPtr.Zero, true, false, null);
            try
            {
                uint written;
                bool ok = NativeMethods.WriteFile(_handle, buffer, (uint)buffer.Length, out written, ref ov);
                if (!ok)
                {
                    int err = Marshal.GetLastWin32Error();
                    if (err != NativeMethods.ERROR_IO_PENDING)
                    {
                        return false;
                    }
                    uint wait = NativeMethods.WaitForSingleObject(ov.hEvent, (uint)timeoutMs);
                    if (wait != NativeMethods.WAIT_OBJECT_0)
                    {
                        NativeMethods.CancelIo(_handle);
                        // 취소가 끝나 버퍼 사용이 멈출 때까지 대기(버퍼 재사용 안전).
                        NativeMethods.GetOverlappedResult(_handle, ref ov, out written, true);
                        return false;
                    }
                    if (!NativeMethods.GetOverlappedResult(_handle, ref ov, out written, false))
                    {
                        return false;
                    }
                }
                return true;
            }
            finally
            {
                NativeMethods.CloseHandle(ov.hEvent);
            }
        }

        /// <summary>입력 리포트 1건 수신. 타임아웃 시 null.</summary>
        public byte[] Read(int timeoutMs)
        {
            int len = InputReportByteLength > 0 ? InputReportByteLength : 64;
            var buffer = new byte[len];

            var ov = new NativeMethods.OVERLAPPED();
            ov.hEvent = NativeMethods.CreateEvent(IntPtr.Zero, true, false, null);
            try
            {
                uint read;
                bool ok = NativeMethods.ReadFile(_handle, buffer, (uint)buffer.Length, out read, ref ov);
                if (!ok)
                {
                    int err = Marshal.GetLastWin32Error();
                    if (err != NativeMethods.ERROR_IO_PENDING)
                    {
                        return null;
                    }
                    uint wait = NativeMethods.WaitForSingleObject(ov.hEvent, (uint)timeoutMs);
                    if (wait != NativeMethods.WAIT_OBJECT_0)
                    {
                        NativeMethods.CancelIo(_handle);
                        // 취소가 끝나 버퍼 사용이 멈출 때까지 대기(버퍼 재사용 안전).
                        NativeMethods.GetOverlappedResult(_handle, ref ov, out read, true);
                        return null;
                    }
                    if (!NativeMethods.GetOverlappedResult(_handle, ref ov, out read, false))
                    {
                        return null;
                    }
                }

                if (read == 0)
                {
                    return null;
                }
                var data = new byte[read];
                Array.Copy(buffer, data, (int)read);
                return data;
            }
            finally
            {
                NativeMethods.CloseHandle(ov.hEvent);
            }
        }

        public void Dispose()
        {
            if (_handle != null && !_handle.IsClosed)
            {
                _handle.Dispose();
            }
        }
    }
}
