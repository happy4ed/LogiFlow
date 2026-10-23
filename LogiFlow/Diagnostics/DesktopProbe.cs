using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Handoff;

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
        [DllImport("kernel32.dll", SetLastError = true)] private static extern bool TerminateProcess(IntPtr h, uint exitCode);
        [DllImport("kernel32.dll", SetLastError = true)] private static extern uint WaitForSingleObject(IntPtr h, uint ms);
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

        /// <summary>세션 0 SYSTEM 에서 호출 — 콘솔 세션 winlogon 데스크톱에 childArgs 로 자신을 띄운다.</summary>
        public static void RunLauncher(string childArgs)
        {
            string who;
            try { who = System.Security.Principal.WindowsIdentity.GetCurrent().Name; }
            catch { who = Environment.UserName; }
            Append("==================================================");
            Append("세션0 런처 시작 · 실행 계정=" + who + " · child=" + childArgs);

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
                string cmd = "\"" + Application.ExecutablePath + "\" " + childArgs;
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
            Append("세션0 런처 종료.");
        }

        // ===== 윈도우 서비스(LogonSwitchService)용 공개 헬퍼 =====

        /// <summary>서비스 supervisor 로그를 워커와 같은 로그 파일(desktop-probe.log)에 남긴다.</summary>
        public static void ServiceLog(string s) { Append("[서비스] " + s); }

        /// <summary>활성 콘솔(물리 콘솔에 연결된) 세션 id. 0xFFFFFFFF 면 미연결.</summary>
        public static uint ActiveConsoleSession() { return WTSGetActiveConsoleSessionId(); }

        /// <summary>CreateProcessAsUser 로 받은 프로세스 핸들이 아직 살아 있나.</summary>
        public static bool ProcessAlive(IntPtr hProcess)
        {
            if (hProcess == IntPtr.Zero) return false;
            return WaitForSingleObject(hProcess, 0) == 0x00000102; // WAIT_TIMEOUT = 아직 실행 중
        }

        /// <summary>프로세스 강제 종료(PID 패턴 매칭 없이 핸들로만) + 핸들 닫기.</summary>
        public static void KillProcess(IntPtr hProcess)
        {
            if (hProcess == IntPtr.Zero) return;
            try { TerminateProcess(hProcess, 0); } catch { }
            try { CloseHandle(hProcess); } catch { }
        }

        /// <summary>
        /// 세션0 SYSTEM(서비스)에서 호출 — 활성 콘솔 세션의 winlogon 데스크톱에 '--logon-switch' 워커를 띄우고
        /// 그 프로세스 핸들을 돌려준다(서비스가 세션 전환 시 KillProcess 로 교체). 실패 시 false.
        /// </summary>
        public static bool SpawnLogonWorker(out IntPtr hProcessOut, out uint pidOut, out uint sessionOut)
        {
            hProcessOut = IntPtr.Zero; pidOut = 0; sessionOut = 0xFFFFFFFF;
            IntPtr hProc = IntPtr.Zero, hTok = IntPtr.Zero, hDup = IntPtr.Zero;
            try
            {
                uint consoleSession = WTSGetActiveConsoleSessionId();
                if (consoleSession == 0xFFFFFFFF) { ServiceLog("활성 콘솔 세션 없음 — 대기"); return false; }
                uint pid = FindProcessInSession("winlogon.exe", consoleSession);
                if (pid == 0) { ServiceLog("winlogon(세션 " + consoleSession + ") 못 찾음 — 대기"); return false; }

                hProc = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid);
                if (hProc == IntPtr.Zero) { ServiceLog("OpenProcess(winlogon) 실패 err=" + Marshal.GetLastWin32Error()); return false; }
                if (!OpenProcessToken(hProc, TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY, out hTok))
                { ServiceLog("OpenProcessToken 실패 err=" + Marshal.GetLastWin32Error()); return false; }
                if (!DuplicateTokenEx(hTok, MAXIMUM_ALLOWED, IntPtr.Zero, SecurityImpersonation, TokenPrimary, out hDup))
                { ServiceLog("DuplicateTokenEx 실패 err=" + Marshal.GetLastWin32Error()); return false; }

                var si = new STARTUPINFO();
                si.cb = Marshal.SizeOf(typeof(STARTUPINFO));
                si.lpDesktop = "winsta0\\winlogon";
                PROCESS_INFORMATION pi;
                string cmd = "\"" + Application.ExecutablePath + "\" --logon-switch";
                bool ok = CreateProcessAsUser(hDup, null, cmd, IntPtr.Zero, IntPtr.Zero, false, CREATE_NO_WINDOW, IntPtr.Zero, null, ref si, out pi);
                if (!ok) { ServiceLog("CreateProcessAsUser 실패 err=" + Marshal.GetLastWin32Error()); return false; }

                CloseHandle(pi.hThread);
                hProcessOut = pi.hProcess;   // 서비스가 소유(교체 시 KillProcess)
                pidOut = pi.dwProcessId;
                sessionOut = consoleSession;
                ServiceLog("워커 투입 pid=" + pi.dwProcessId + " 세션=" + consoleSession);
                return true;
            }
            catch (Exception ex) { ServiceLog("워커 투입 오류: " + ex.Message); return false; }
            finally
            {
                if (hDup != IntPtr.Zero) CloseHandle(hDup);
                if (hTok != IntPtr.Zero) CloseHandle(hTok);
                if (hProc != IntPtr.Zero) CloseHandle(hProc);
            }
        }

        // ===== 로그온/잠금 화면 전환 워커 (winlogon 데스크톱 · SYSTEM · 실제 전환) =====
        private sealed class EdgeDet
        {
            public EdgeDetector Det; public HandoffRect Rect; public HandoffEdge Edge; public int Channel; public string Label;
        }

        /// <summary>winlogon 데스크톱에서 호출 — 보안 화면(잠금/로그온)일 때만 엣지 감지→setHost. 무한 실행.</summary>
        public static void RunLogonSwitch()
        {
            // 단일 인스턴스 — 중복 워커가 HID 를 서로 물면 setHost 가 0개로 실패한다(실측). 하나만 돈다.
            bool createdNew = false;
            Mutex mutex = null;
            try { mutex = new Mutex(true, "Global\\LogiFlow-LogonSwitch-Worker-7A2", out createdNew); }
            catch { createdNew = true; }
            if (!createdNew)
            {
                Append("logon-switch 워커가 이미 실행 중 — 중복 인스턴스 종료(HID 충돌 방지).");
                return;
            }
            try
            {
                var worker = new Thread(LogonSwitchLoop);
                try { worker.SetApartmentState(ApartmentState.MTA); } catch { }
                worker.IsBackground = false;
                worker.Start();
                worker.Join();
            }
            finally { try { if (mutex != null) mutex.ReleaseMutex(); } catch { } GC.KeepAlive(mutex); }
        }

        private static void LogonSwitchLoop()
        {
            string who;
            try { who = System.Security.Principal.WindowsIdentity.GetCurrent().Name; } catch { who = Environment.UserName; }
            Append("==================================================");
            Append("logon-switch 워커 시작 · 계정=" + who);

            int dwellMs, myHost; bool debug;
            List<LogonEdge> cfgEdges = LogonEdges.Read(out dwellMs, out myHost, out debug);
            Append("설정 파일 = " + LogonEdges.FilePath + " · 디버그=" + (debug ? "켜짐" : "꺼짐"));

            var hlog = new DiagLogger(AppDomain.CurrentDomain.BaseDirectory) { Enabled = true };
            var switchable = new List<ScanResult>();
            try
            {
                var results = new DeviceScanner(new DiagLogger(AppDomain.CurrentDomain.BaseDirectory) { Enabled = false }).Scan();
                foreach (var r in results) if (r.ChangeHostSupported && r.HostCount > 0) switchable.Add(r);
            }
            catch (Exception ex) { Append("기기 스캔 오류: " + ex.Message); }
            Append(string.Format("엣지 {0}개 · 전환가능 기기 {1}개 · dwell={2}ms", cfgEdges.Count, switchable.Count, dwellMs));

            // 콜드부팅 로그인 전엔 로지텍 기기가 자고 있어 스캔이 0~1개로 흔들린다(로그인 후 2개).
            // 전환 순간에 느린 재스캔(≈9초)을 하면 지연·누락이 생기므로, 백그라운드로 주기 스캔해
            // '깨어난 기기'를 미리 목록에 채워 둔다. 전송은 이 최신 목록에서 '즉시' 나간다.
            var scanLock = new object();
            var sharedBox = new List<ScanResult>[] { switchable };   // 최신 전환가능 목록
            var activeBox = new bool[] { false };                     // 로그온/잠금 화면일 때만 스캔
            var scanner = new Thread(() =>
            {
                long last = 0;
                while (true)
                {
                    Thread.Sleep(200);
                    bool act; lock (scanLock) act = activeBox[0];
                    if (!act) continue;   // 사용자 바탕화면에선 스캔 안 함(트레이 앱과 HID 경합 방지)
                    int have; lock (scanLock) have = sharedBox[0].Count;
                    long t = Environment.TickCount;
                    // 기기가 있으면 5초마다 갱신, 없으면 1.5초마다 재탐색(깨어나길 계속 찾는다).
                    if (have > 0 ? (t - last < 5000) : (t - last < 1500)) continue;
                    last = t;
                    try
                    {
                        var rr = new DeviceScanner(new DiagLogger(AppDomain.CurrentDomain.BaseDirectory) { Enabled = false }).Scan();
                        var list = new List<ScanResult>();
                        foreach (var r in rr) if (r.ChangeHostSupported && r.HostCount > 0) list.Add(r);
                        if (list.Count > 0) { lock (scanLock) sharedBox[0] = list; }   // 0이면 직전 목록 유지(일시적 누락 무시)
                    }
                    catch { }
                }
            });
            try { scanner.SetApartmentState(ApartmentState.MTA); } catch { }
            scanner.IsBackground = true;
            scanner.Start();
            int lastLoggedCount = switchable.Count;

            Screen[] scr = Screen.AllScreens;
            if (debug)
            {
                Append("모니터 " + scr.Length + "개(워커 기준 좌표):");
                for (int i = 0; i < scr.Length; i++)
                    Append(string.Format("  [{0}] {1} bounds=({2},{3} {4}x{5}){6}",
                        i, scr[i].DeviceName, scr[i].Bounds.Left, scr[i].Bounds.Top, scr[i].Bounds.Width, scr[i].Bounds.Height,
                        scr[i].Primary ? " 주" : ""));
            }
            var dets = new List<EdgeDet>();
            foreach (var e in cfgEdges)
            {
                Screen s = null;
                foreach (var sc in scr) if (sc.DeviceName == e.Device) { s = sc; break; }
                if (s == null && e.Index >= 0 && e.Index < scr.Length) s = scr[e.Index];
                if (s == null) { Append("엣지 매칭 실패(모니터 없음): " + e.Device + " idx=" + e.Index); continue; }
                var b = s.Bounds;
                var det = new EdgeDet
                {
                    // 워커는 GetCursorPos 폴링이라 화면 끝에 박힌 커서의 '바깥 밀기'를 못 읽는다 →
                    // MinPushPx=0 으로 '변에서 dwell 채우면 전환'. (사용자 세션은 raw 입력이라 밀기 요건 유지)
                    Det = new EdgeDetector { DwellMs = dwellMs, MinPushPx = 0 },
                    Rect = new HandoffRect(b.Left, b.Top, b.Width, b.Height),
                    Edge = (HandoffEdge)e.Edge,
                    Channel = e.Channel,
                    Label = s.DeviceName + " " + (HandoffEdge)e.Edge
                };
                dets.Add(det);
                if (debug) Append(string.Format("엣지 등록: {0} mon=({1},{2} {3}x{4}) → 채널 {5}",
                    det.Label, b.Left, b.Top, b.Width, b.Height, det.Channel + 1));
            }
            if (dets.Count == 0) Append("※ 적용할 엣지 0개 — [전환 관리]에서 채널 박스를 변에 붙였는지 확인하세요.");

            IntPtr original = GetThreadDesktop(GetCurrentThreadId());
            int lastX = 0, lastY = 0; bool haveLast = false;
            long lastSwitch = 0, lastDebug = 0, lastConfigRead = 0;
            string lastName = null;

            while (true)
            {
                Thread.Sleep(50);
                IntPtr hIn = OpenInputDesktop(0, false, DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS);
                if (hIn == IntPtr.Zero) { haveLast = false; continue; }
                string name = DesktopName(hIn);
                bool secure = !string.Equals(name, "Default", StringComparison.OrdinalIgnoreCase);
                bool locked = LogonEdges.IsLocked();
                // 전환 감지 ON 조건: 보안 데스크톱(로그온 화면) 또는 세션 잠김(ARM 은 잠금 때도 Default 로 보임).
                bool active = secure || locked;
                lock (scanLock) activeBox[0] = active;   // 백그라운드 스캐너 on/off
                POINT p = new POINT(); bool got = false;
                // active 또는 디버그면 커서를 읽는다(일반 상태 디버그 로깅 포함).
                if (active || debug)
                {
                    if (SetThreadDesktop(hIn)) got = GetCursorPos(out p);
                    if (original != IntPtr.Zero) SetThreadDesktop(original);
                }
                CloseDesktop(hIn);

                long now = Environment.TickCount;
                // 디버그 토글을 재부팅 없이 반영 — 설정 파일을 주기적으로 재읽기.
                if (now - lastConfigRead > 4000)
                {
                    lastConfigRead = now;
                    int d1, m1; bool dbg;
                    LogonEdges.Read(out d1, out m1, out dbg);
                    if (dbg != debug) { debug = dbg; Append("디버그 모드 " + (debug ? "켜짐" : "꺼짐") + " (설정 반영)"); }
                }
                if (name != lastName)
                {
                    Append("입력 데스크톱=" + name + " · 잠김=" + (locked ? "Y" : "N") + (active ? " → 전환 감지 ON" : " → OFF(사용자 세션 담당)"));
                    lastName = name;
                }
                // 백그라운드 스캔으로 전환가능 기기 수가 바뀌면 알린다(기기가 깨어나는 과정 가시화).
                int curCount; lock (scanLock) curCount = sharedBox[0].Count;
                if (curCount != lastLoggedCount)
                {
                    Append("전환가능 기기 " + lastLoggedCount + "개 → " + curCount + "개 (백그라운드 스캔)");
                    lastLoggedCount = curCount;
                }

                if (debug && got && now - lastDebug > 400)
                {
                    var sb = new StringBuilder();
                    sb.Append(string.Format("desktop={0} secure={1} locked={2} active={3} cursor=({4},{5})",
                        name, secure ? "Y" : "N", locked ? "Y" : "N", active ? "Y" : "N", p.X, p.Y));
                    foreach (var d in dets)
                    {
                        int dist = EdgeDetector.DistanceFromEdge(d.Rect, d.Edge, p.X, p.Y);
                        sb.Append(string.Format(" | {0} dist={1} band={2} dwell={3:0.00}",
                            d.Label, dist, dist <= EdgeDetector.DefaultBandPx ? "Y" : "N", d.Det.Progress(now)));
                    }
                    Append(sb.ToString());
                    lastDebug = now;
                }

                if (!active || !got) { haveLast = false; foreach (var d in dets) d.Det.Reset(true); continue; }

                int dx = haveLast ? p.X - lastX : 0, dy = haveLast ? p.Y - lastY : 0;
                lastX = p.X; lastY = p.Y; haveLast = true;
                if (now - lastSwitch < 1200) continue;

                foreach (var d in dets)
                {
                    int push = EdgeDetector.OutwardPush(d.Edge, dx, dy);
                    if (d.Det.Feed(d.Rect, d.Edge, p.X, p.Y, push, now, false, false))
                    {
                        Append(string.Format("[로그온 전환] {0} → 채널 {1} · cursor=({2},{3})", d.Label, d.Channel + 1, p.X, p.Y));
                        // 백그라운드 스캐너가 유지하는 최신 목록에서 즉시 전송(느린 인라인 재스캔 제거 — 9초 지연 없음).
                        List<ScanResult> sw; lock (scanLock) sw = sharedBox[0];
                        if (sw.Count == 0)
                            Append("  전환가능 기기 0개 — 마우스·키보드가 아직 자고 있음(한 번 움직여/눌러 깨우면 다음 밀기에 전환).");
                        // 기기별 쓰기 결과를 '보이는 로그'에 남기고, 콜드부팅 대비 확인·재시도(최대 3회).
                        try { int ok = HostSwitcher.SwitchAllVerbose(sw, d.Channel, hlog, Append, 3); Append("  setHost 전환 " + ok + "/" + sw.Count + "개"); }
                        catch (Exception ex) { Append("  setHost 오류: " + ex.Message); }
                        lastSwitch = now;
                        foreach (var o in dets) o.Det.Reset(true);
                        break;
                    }
                }
            }
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
                { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true };
                using (var p = Process.Start(psi))
                {
                    if (p == null) return false;
                    p.StandardOutput.ReadToEnd();
                    p.StandardError.ReadToEnd();
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
