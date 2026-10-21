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
            string inner = "\\\"" + exe + "\\\" --desktop-probe=120";
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
