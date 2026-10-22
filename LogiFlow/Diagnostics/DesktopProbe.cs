using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

namespace LogiFlow.Diagnostics
{
    /// <summary>
    /// 로그온/잠금 화면(보안 데스크톱=Winlogon)에서 커서를 읽을 수 있는지 '가능성만' 확인하는
    /// 읽기 전용 프로브. HID 접근·setHost 는 전혀 하지 않는다(쓰기 0). SYSTEM 권한에서 돌려야 의미가 있으며,
    /// 예약 작업(ONSTART SYSTEM)으로 설치·실행한다. 동작:
    ///  - 매 주기 OpenInputDesktop 으로 '현재 입력 데스크톱'(로그인 시 Winlogon)을 열고 이름을 읽는다.
    ///  - 전용 스레드(창/훅 없음)를 그 데스크톱에 SetThreadDesktop 으로 붙인 뒤 GetCursorPos 로 커서를 읽는다.
    ///  - 성공/실패·오류코드·데스크톱 이름·커서 좌표를 로그로 남긴다 → 회장님이 로그를 열어 가능성 판정.
    /// MS 공식 문서 패턴(GetCursorPos 비고: OpenInputDesktop→SetThreadDesktop)을 그대로 따른다.
    /// </summary>
    internal static class DesktopProbe
    {
        private const uint DESKTOP_READOBJECTS = 0x0001;
        private const uint DESKTOP_WRITEOBJECTS = 0x0080;
        private const int UOI_NAME = 2;

        [StructLayout(LayoutKind.Sequential)]
        private struct POINT { public int X; public int Y; }

        [DllImport("user32.dll", SetLastError = true)]
        private static extern IntPtr OpenInputDesktop(uint dwFlags, bool fInherit, uint dwDesiredAccess);
        [DllImport("user32.dll", SetLastError = true)]
        private static extern IntPtr GetThreadDesktop(uint dwThreadId);
        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool SetThreadDesktop(IntPtr hDesktop);
        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool CloseDesktop(IntPtr hDesktop);
        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool GetCursorPos(out POINT lpPoint);
        [DllImport("kernel32.dll")]
        private static extern uint GetCurrentThreadId();
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern bool GetUserObjectInformation(IntPtr hObj, int nIndex, byte[] pvInfo, int nLength, out int lpnLengthNeeded);

        /// <summary>프로브 로그 경로(%ProgramData%\LogiFlow\desktop-probe.log · SYSTEM 쓰기·사용자 읽기).</summary>
        public static string LogPath
        {
            get
            {
                string dir = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "LogiFlow");
                return Path.Combine(dir, "desktop-probe.log");
            }
        }

        private static void Append(string line)
        {
            try
            {
                string path = LogPath;
                Directory.CreateDirectory(Path.GetDirectoryName(path));
                File.AppendAllText(path, DateTime.Now.ToString("HH:mm:ss.fff") + "  " + line + Environment.NewLine, Encoding.UTF8);
            }
            catch { }
        }

        private static string DesktopName(IntPtr hDesk)
        {
            try
            {
                var buf = new byte[256];
                int needed;
                if (GetUserObjectInformation(hDesk, UOI_NAME, buf, buf.Length, out needed))
                {
                    string s = Encoding.Unicode.GetString(buf, 0, Math.Max(0, Math.Min(buf.Length, needed)));
                    int z = s.IndexOf('\0');
                    if (z >= 0) s = s.Substring(0, z);
                    return s;
                }
            }
            catch { }
            return "(이름?)";
        }

        /// <summary>프로브 본체. seconds 동안 입력 데스크톱 커서 읽기를 시도·기록하고 종료.</summary>
        public static void RunProbe(int seconds)
        {
            if (seconds < 5) seconds = 5;
            if (seconds > 600) seconds = 600;

            string who;
            try { who = System.Security.Principal.WindowsIdentity.GetCurrent().Name; }
            catch { who = Environment.UserName; }

            Append("==================================================");
            Append(string.Format("desktop-probe 시작 · 실행 계정={0} · {1}초 · 지금부터 잠금(Win+L) 또는 로그인 화면에서 마우스를 움직여 보세요", who, seconds));

            // SetThreadDesktop 은 '창·훅 없는' 스레드에서만 성공한다. [STAThread] 메인 스레드는 .NET 이 STA COM
            // 용 숨은 창을 만들어 err170(ERROR_BUSY)로 실패하므로, 데스크톱/커서 작업은 새 MTA 워커에서 수행한다.
            var worker = new Thread(() => ProbeLoop(seconds));
            worker.IsBackground = false;
            try { worker.SetApartmentState(ApartmentState.MTA); } catch { }
            worker.Start();
            worker.Join((seconds + 10) * 1000);
        }

        private static void ProbeLoop(int seconds)
        {
            // 이 컨텍스트(SYSTEM·winlogon 데스크톱)에서 HID(=setHost 가 쓰는 경로)가 읽히는지 1회 테스트.
            // 커서 읽기는 이미 됨 → 남은 변수는 '여기서 HID 열기/전환이 되는가'. ARM 잠금화면 전환 실패 원인 후보를 가린다.
            // 읽기 전용(열거 + 0x1814 조회만, setHost 는 보내지 않음).
            try
            {
                var dlog = new DiagLogger(AppDomain.CurrentDomain.BaseDirectory) { Enabled = false };
                var results = new DeviceScanner(dlog).Scan();
                int sw = 0;
                foreach (var r in results) if (r.ChangeHostSupported) sw++;
                Append(string.Format("--- HID 접근 테스트: 기기 {0}개 · 전환가능(0x1814) {1}개 ---", results.Count, sw));
                foreach (var r in results)
                {
                    Append(string.Format("  {0} [{1}] 0x1814={2} 현재채널={3} {4}",
                        r.DeviceName, r.VidPid,
                        r.ChangeHostSupported ? "0x" + r.ChangeHostIndex.ToString("X2") : "미지원",
                        r.CurrentHost >= 0 ? (r.CurrentHost + 1).ToString() : "?",
                        string.IsNullOrEmpty(r.Note) ? "" : "(" + r.Note + ")"));
                }
                Append(sw > 0
                    ? "→ 이 컨텍스트에서 HID 읽기 OK. setHost(전환)도 같은 경로라 로그온 화면 전환 구현 가능."
                    : "→ 이 컨텍스트에서 전환 가능 기기를 못 읽음 — 여기가 ARM/잠금화면 전환 실패의 원인 후보.");
            }
            catch (Exception ex) { Append("HID 접근 테스트 오류: " + ex.Message); }

            IntPtr original = GetThreadDesktop(GetCurrentThreadId());
            long end = Environment.TickCount + seconds * 1000;
            string lastName = null;
            int tick = 0;

            while (Environment.TickCount < end)
            {
                tick++;
                IntPtr hIn = OpenInputDesktop(0, false, DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS);
                if (hIn == IntPtr.Zero)
                {
                    Append("OpenInputDesktop 실패 (입력 데스크톱 접근 거부) err=" + Marshal.GetLastWin32Error());
                    Thread.Sleep(700);
                    continue;
                }

                string name = DesktopName(hIn);
                bool setOk = SetThreadDesktop(hIn);
                int setErr = setOk ? 0 : Marshal.GetLastWin32Error();

                bool gotCur = false; int curErr = 0; POINT p = new POINT();
                if (setOk) { gotCur = GetCursorPos(out p); curErr = gotCur ? 0 : Marshal.GetLastWin32Error(); }

                if (name != lastName)
                {
                    Append(">>> 현재 입력 데스크톱 = \"" + name + "\"  (Winlogon = 로그온/잠금 보안 화면)");
                    lastName = name;
                }
                // 매 줄은 간결히(5줄마다 한 번만 상세).
                if (tick % 3 == 0 || !setOk || !gotCur)
                {
                    Append(string.Format("desktop=\"{0}\" setThreadDesktop={1}(err{2}) GetCursorPos={3}(err{4})",
                        name, setOk ? "성공" : "실패", setErr,
                        gotCur ? string.Format("({0},{1})", p.X, p.Y) : "실패", curErr));
                }

                // 원래 데스크톱으로 되돌린 뒤 핸들 닫기(현재 데스크톱 닫기 방지).
                if (setOk && original != IntPtr.Zero) SetThreadDesktop(original);
                CloseDesktop(hIn);
                Thread.Sleep(600);
            }

            Append("desktop-probe 종료. 위에서 desktop=\"Winlogon\" 줄의 GetCursorPos 가 '성공(좌표 변함)'이면 로그인 화면 엣지 전환 구현 가능.");
        }

        // ===== 2차: 세션 0(SYSTEM) → 콘솔 세션 winlogon 데스크톱으로 프로브를 띄우는 런처 =====
        // 세션 0 서비스에는 입력 데스크톱이 없어 1차(직접 OpenInputDesktop)는 err=1 로 실패한다.
        // winlogon.exe(콘솔 세션·SYSTEM) 토큰을 복제해 winsta0\winlogon 데스크톱에 --desktop-probe 를 CreateProcessAsUser.

        private const uint PROCESS_QUERY_INFORMATION = 0x0400;
        private const uint TOKEN_DUPLICATE = 0x0002, TOKEN_QUERY = 0x0008, TOKEN_ASSIGN_PRIMARY = 0x0001;
        private const uint MAXIMUM_ALLOWED = 0x02000000;
        private const int SecurityImpersonation = 2;
        private const int TokenPrimary = 1;
        private const uint CREATE_NO_WINDOW = 0x08000000;
        private const uint TH32CS_SNAPPROCESS = 0x00000002;

        [DllImport("kernel32.dll")] private static extern uint WTSGetActiveConsoleSessionId();
        [DllImport("kernel32.dll", SetLastError = true)] private static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
        [DllImport("kernel32.dll", SetLastError = true)] private static extern bool CloseHandle(IntPtr h);
        [DllImport("kernel32.dll", SetLastError = true)] private static extern bool ProcessIdToSessionId(uint pid, out uint sessionId);
        [DllImport("kernel32.dll", SetLastError = true)] private static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint pid);
        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)] private static extern bool Process32First(IntPtr snap, ref PROCESSENTRY32 pe);
        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)] private static extern bool Process32Next(IntPtr snap, ref PROCESSENTRY32 pe);
        [DllImport("advapi32.dll", SetLastError = true)] private static extern bool OpenProcessToken(IntPtr proc, uint access, out IntPtr token);
        [DllImport("advapi32.dll", SetLastError = true)] private static extern bool DuplicateTokenEx(IntPtr existing, uint access, IntPtr attrs, int impLevel, int tokenType, out IntPtr newToken);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)] private static extern bool CreateProcessAsUser(IntPtr token, string app, string cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string curDir, ref STARTUPINFO si, out PROCESS_INFORMATION pi);

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct PROCESSENTRY32
        {
            public uint dwSize; public uint cntUsage; public uint th32ProcessID; public IntPtr th32DefaultHeapID;
            public uint th32ModuleID; public uint cntThreads; public uint th32ParentProcessID; public int pcPriClassBase;
            public uint dwFlags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string szExeFile;
        }
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct STARTUPINFO
        {
            public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
            public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
            public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
        }
        [StructLayout(LayoutKind.Sequential)]
        private struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public uint dwProcessId, dwThreadId; }

        private static uint FindProcessInSession(string exeName, uint session)
        {
            IntPtr snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if (snap == IntPtr.Zero || snap == new IntPtr(-1)) return 0;
            try
            {
                var pe = new PROCESSENTRY32 { dwSize = (uint)Marshal.SizeOf(typeof(PROCESSENTRY32)) };
                if (!Process32First(snap, ref pe)) return 0;
                do
                {
                    if (string.Equals(pe.szExeFile, exeName, StringComparison.OrdinalIgnoreCase))
                    {
                        uint sid;
                        if (ProcessIdToSessionId(pe.th32ProcessID, out sid) && sid == session)
                            return pe.th32ProcessID;
                    }
                } while (Process32Next(snap, ref pe));
            }
            finally { CloseHandle(snap); }
            return 0;
        }

        /// <summary>세션 0 SYSTEM 에서 호출 — 콘솔 세션 winlogon 데스크톱에 프로브를 띄운다.</summary>
        public static void RunLauncher()
        {
            string who;
            try { who = System.Security.Principal.WindowsIdentity.GetCurrent().Name; }
            catch { who = Environment.UserName; }
            Append("==================================================");
            Append("desktop-probe-launch(세션0 런처) 시작 · 실행 계정=" + who);

            try
            {
                uint consoleSession = WTSGetActiveConsoleSessionId();
                Append("활성 콘솔 세션 = " + consoleSession + " (0xFFFFFFFF 면 콘솔 미연결)");

                uint pid = FindProcessInSession("winlogon.exe", consoleSession);
                if (pid == 0) { Append("winlogon.exe(세션 " + consoleSession + ") 을 찾지 못함 — 중단"); return; }
                Append("winlogon.exe pid=" + pid);

                IntPtr hProc = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid);
                if (hProc == IntPtr.Zero) { Append("OpenProcess(winlogon) 실패 err=" + Marshal.GetLastWin32Error()); return; }

                IntPtr hTok;
                if (!OpenProcessToken(hProc, TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY, out hTok))
                { Append("OpenProcessToken 실패 err=" + Marshal.GetLastWin32Error()); CloseHandle(hProc); return; }

                IntPtr hDup;
                if (!DuplicateTokenEx(hTok, MAXIMUM_ALLOWED, IntPtr.Zero, SecurityImpersonation, TokenPrimary, out hDup))
                { Append("DuplicateTokenEx 실패 err=" + Marshal.GetLastWin32Error()); CloseHandle(hTok); CloseHandle(hProc); return; }

                var si = new STARTUPINFO();
                si.cb = Marshal.SizeOf(typeof(STARTUPINFO));
                si.lpDesktop = "winsta0\\winlogon";   // ← 로그인/보안 데스크톱
                PROCESS_INFORMATION pi;
                string cmd = "\"" + Application.ExecutablePath + "\" --desktop-probe=100";
                bool ok = CreateProcessAsUser(hDup, null, cmd, IntPtr.Zero, IntPtr.Zero, false, CREATE_NO_WINDOW, IntPtr.Zero, null, ref si, out pi);
                if (!ok)
                {
                    Append("CreateProcessAsUser(winsta0\\winlogon) 실패 err=" + Marshal.GetLastWin32Error()
                        + " → 세션 1 winlogon 데스크톱 진입 불가(정책/권한)");
                }
                else
                {
                    Append("CreateProcessAsUser 성공 — 세션 " + consoleSession + " winlogon 데스크톱에 프로브 띄움 pid=" + pi.dwProcessId
                        + ". 이제 Win+L 로 잠금화면에서 마우스를 움직여 보세요(약 100초). 아래에 그 프로브 로그가 이어집니다.");
                    CloseHandle(pi.hProcess); CloseHandle(pi.hThread);
                }

                CloseHandle(hDup); CloseHandle(hTok); CloseHandle(hProc);
            }
            catch (Exception ex)
            {
                Append("런처 오류: " + ex);
            }
            Append("desktop-probe-launch 종료.");
        }

        public static void OpenLog()
        {
            try
            {
                string path = LogPath;
                if (!File.Exists(path))
                {
                    MessageBox.Show("아직 테스트 로그가 없습니다.\r\n먼저 [테스트 설치·실행] 후, 잠금화면(Win+L)에서 마우스를 움직여 보세요.",
                        "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Information);
                    return;
                }
                Process.Start("notepad.exe", "\"" + path + "\"");
            }
            catch (Exception ex)
            {
                MessageBox.Show("로그를 열지 못했습니다: " + ex.Message, "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            }
        }
    }

    /// <summary>
    /// 프로브를 SYSTEM 권한으로 돌리기 위한 예약 작업(ONSTART SYSTEM) 설치/실행/제거.
    /// 설치·실행은 관리자 권한이 필요하므로 자기 자신을 runas 로 상승해 schtasks 를 실행한다.
    /// 이 작업이 '설치되는지' 자체가 회장님 관리형 PC 정책 통과 여부의 1차 판정.
    /// </summary>
    internal static class ProbeTask
    {
        public const string TaskName = "LogiFlow-DesktopProbe";

        public static bool IsInstalled()
        {
            try
            {
                var psi = new ProcessStartInfo("schtasks.exe", "/Query /TN \"" + TaskName + "\"")
                { UseShellExecute = false, CreateNoWindow = true };
                using (var p = Process.Start(psi))
                {
                    if (p == null) return false;
                    if (!p.WaitForExit(5000)) { try { p.Kill(); } catch { } return false; }
                    return p.ExitCode == 0;
                }
            }
            catch { return false; }
        }

        /// <summary>설치 + 즉시 실행(상승). 성공 true.</summary>
        public static bool RequestInstallRun() { return RunElevated("--install-probe"); }
        public static bool RequestUninstall() { return RunElevated("--uninstall-probe"); }

        private static bool RunElevated(string arg)
        {
            try
            {
                var psi = new ProcessStartInfo(Application.ExecutablePath, arg) { UseShellExecute = true, Verb = "runas" };
                using (var p = Process.Start(psi))
                {
                    if (p == null) return false;
                    p.WaitForExit(20000);
                    return p.ExitCode == 0;
                }
            }
            catch (Win32Exception) { return false; }
            catch { return false; }
        }

        // ---- 상승된 인스턴스에서 실행 ----
        public static int DoInstallRun()
        {
            string exe = Application.ExecutablePath;
            // 세션 0 런처를 띄운다(런처가 콘솔 세션 winlogon 데스크톱으로 프로브를 재투입).
            string inner = "\\\"" + exe + "\\\" --desktop-probe-launch";
            int rc = RunSchtasks("/Create /TN \"" + TaskName + "\" /TR \"" + inner + "\" /SC ONSTART /RU SYSTEM /RL HIGHEST /F");
            if (rc != 0) return rc;
            // 지금 즉시 한 번 실행(잠금화면 테스트용).
            RunSchtasks("/Run /TN \"" + TaskName + "\"");
            return 0;
        }

        public static int DoUninstall() { return RunSchtasks("/Delete /TN \"" + TaskName + "\" /F"); }

        private static int RunSchtasks(string args)
        {
            try
            {
                var psi = new ProcessStartInfo("schtasks.exe", args) { UseShellExecute = false, CreateNoWindow = true };
                using (var p = Process.Start(psi))
                {
                    if (p == null) return 1;
                    p.WaitForExit(15000);
                    return p.ExitCode;
                }
            }
            catch { return 1; }
        }
    }
}
