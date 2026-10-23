using System;
using System.Runtime.InteropServices;

namespace LogiFlow.Service
{
    /// <summary>
    /// 윈도우 서비스 설치/삭제/존재확인 — advapi32 직접 호출(sc.exe 따옴표 문제 회피).
    /// 설치·삭제는 관리자 권한 필요(상승된 --install-prelogon / --uninstall-prelogon 에서 호출).
    /// 존재확인은 표준 사용자 권한(SC_MANAGER_CONNECT)으로도 된다(토글 상태 표시용).
    /// </summary>
    internal static class ServiceControl
    {
        private const uint SC_MANAGER_CONNECT = 0x0001;
        private const uint SC_MANAGER_ALL_ACCESS = 0xF003F;
        private const uint SERVICE_ALL_ACCESS = 0xF01FF;
        private const uint SERVICE_QUERY_STATUS = 0x0004;
        private const uint SERVICE_WIN32_OWN_PROCESS = 0x00000010;
        private const uint SERVICE_AUTO_START = 0x00000002;
        private const uint SERVICE_ERROR_NORMAL = 0x00000001;
        private const uint SERVICE_CONTROL_STOP = 0x00000001;
        private const int ERROR_SERVICE_EXISTS = 1073;
        private const int ERROR_SERVICE_MARKED_FOR_DELETE = 1072;

        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern IntPtr OpenSCManager(string machine, string database, uint access);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern IntPtr CreateService(IntPtr scm, string name, string display, uint access,
            uint serviceType, uint startType, uint errorControl, string binPath, string loadGroup,
            IntPtr tagId, string dependencies, string serviceStartName, string password);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern IntPtr OpenService(IntPtr scm, string name, uint access);
        [DllImport("advapi32.dll", SetLastError = true)]
        private static extern bool StartService(IntPtr svc, uint numArgs, string[] args);
        [DllImport("advapi32.dll", SetLastError = true)]
        private static extern bool ControlService(IntPtr svc, uint control, ref SERVICE_STATUS status);
        [DllImport("advapi32.dll", SetLastError = true)]
        private static extern bool DeleteService(IntPtr svc);
        [DllImport("advapi32.dll", SetLastError = true)]
        private static extern bool CloseServiceHandle(IntPtr h);

        [StructLayout(LayoutKind.Sequential)]
        private struct SERVICE_STATUS
        {
            public uint dwServiceType, dwCurrentState, dwControlsAccepted, dwWin32ExitCode,
                dwServiceSpecificExitCode, dwCheckPoint, dwWaitHint;
        }

        /// <summary>서비스가 등록돼 있나(표준 사용자 권한으로 확인).</summary>
        public static bool Exists()
        {
            IntPtr scm = OpenSCManager(null, null, SC_MANAGER_CONNECT);
            if (scm == IntPtr.Zero) return false;
            try
            {
                IntPtr svc = OpenService(scm, LogonSwitchService.SvcName, SERVICE_QUERY_STATUS);
                if (svc == IntPtr.Zero) return false;
                CloseServiceHandle(svc);
                return true;
            }
            finally { CloseServiceHandle(scm); }
        }

        /// <summary>설치 + 즉시 시작(관리자). 0 성공, 그 외 Win32 오류코드.</summary>
        public static int Install(string exePath)
        {
            IntPtr scm = OpenSCManager(null, null, SC_MANAGER_ALL_ACCESS);
            if (scm == IntPtr.Zero) return 5; // ERROR_ACCESS_DENIED 추정
            try
            {
                string bin = "\"" + exePath + "\" --service";
                IntPtr svc = CreateService(scm, LogonSwitchService.SvcName, "LogiFlow 로그온 전환",
                    SERVICE_ALL_ACCESS, SERVICE_WIN32_OWN_PROCESS, SERVICE_AUTO_START, SERVICE_ERROR_NORMAL,
                    bin, null, IntPtr.Zero, null, null /*LocalSystem*/, null);
                if (svc == IntPtr.Zero)
                {
                    int err = Marshal.GetLastWin32Error();
                    if (err == ERROR_SERVICE_EXISTS)
                    {
                        // 이미 있으면 열어서 시작만(다음 부팅 자동 상주가 목적).
                        svc = OpenService(scm, LogonSwitchService.SvcName, SERVICE_ALL_ACCESS);
                        if (svc == IntPtr.Zero) return ERROR_SERVICE_EXISTS;
                    }
                    else return err;
                }
                try { StartService(svc, 0, null); } catch { }   // 실패해도 다음 부팅에 자동 시작됨
                CloseServiceHandle(svc);
                return 0;
            }
            finally { CloseServiceHandle(scm); }
        }

        /// <summary>중지 + 삭제(관리자). 0 성공(또는 이미 없음).</summary>
        public static int Uninstall()
        {
            IntPtr scm = OpenSCManager(null, null, SC_MANAGER_ALL_ACCESS);
            if (scm == IntPtr.Zero) return 5;
            try
            {
                IntPtr svc = OpenService(scm, LogonSwitchService.SvcName, SERVICE_ALL_ACCESS);
                if (svc == IntPtr.Zero) return 0; // 이미 없음
                try { var st = new SERVICE_STATUS(); ControlService(svc, SERVICE_CONTROL_STOP, ref st); } catch { }
                bool ok = DeleteService(svc);
                int err = ok ? 0 : Marshal.GetLastWin32Error();
                CloseServiceHandle(svc);
                if (err == ERROR_SERVICE_MARKED_FOR_DELETE) err = 0; // 사실상 성공
                return err;
            }
            finally { CloseServiceHandle(scm); }
        }
    }
}
