using System;
using System.Runtime.InteropServices;

namespace LogiFlow.Hid
{
    /// <summary>hidraw 에 필요한 libc 호출만. x86_64·aarch64 상수 동일.</summary>
    internal static class Libc
    {
        public const int O_RDWR = 0x2;
        public const int O_NONBLOCK = 0x800;
        public const int O_CLOEXEC = 0x80000;

        public const int EINTR = 4;
        public const int EAGAIN = 11;
        public const int EACCES = 13;
        public const int EPERM = 1;

        public const short POLLIN = 0x1;
        public const short POLLOUT = 0x4;

        [StructLayout(LayoutKind.Sequential)]
        public struct PollFd
        {
            public int Fd;
            public short Events;
            public short Revents;
        }

        [DllImport("libc", SetLastError = true)]
        public static extern int open([MarshalAs(UnmanagedType.LPUTF8Str)] string path, int flags);

        [DllImport("libc", SetLastError = true)]
        public static extern int close(int fd);

        [DllImport("libc", SetLastError = true)]
        public static extern IntPtr read(int fd, byte[] buf, UIntPtr count);

        [DllImport("libc", SetLastError = true)]
        public static extern IntPtr write(int fd, byte[] buf, UIntPtr count);

        [DllImport("libc", SetLastError = true)]
        public static extern int poll([In, Out] PollFd[] fds, UIntPtr nfds, int timeout);

        [DllImport("libc", SetLastError = true)]
        private static extern IntPtr realpath([MarshalAs(UnmanagedType.LPUTF8Str)] string path, IntPtr resolved);

        [DllImport("libc")]
        private static extern void free(IntPtr p);

        /// <summary>심볼릭 링크를 끝까지 풀어 절대 경로로. 실패 시 null.</summary>
        public static string RealPath(string path)
        {
            IntPtr p = realpath(path, IntPtr.Zero);
            if (p == IntPtr.Zero) return null;
            try { return Marshal.PtrToStringUTF8(p); }
            finally { free(p); }
        }

        /// <summary>fd 하나를 events 로 timeoutMs 까지 기다린다. 준비되면 true.</summary>
        public static bool PollOne(int fd, short events, int timeoutMs)
        {
            var fds = new[] { new PollFd { Fd = fd, Events = events } };
            while (true)
            {
                int r = poll(fds, (UIntPtr)1, timeoutMs);
                if (r > 0) return (fds[0].Revents & events) != 0;
                if (r == 0) return false;
                if (Marshal.GetLastWin32Error() != EINTR) return false;
            }
        }
    }
}
