using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Windows.Forms;
using LogiFlow.Diagnostics;

namespace LogiFlow.Config
{
    /// <summary>
    /// 로그온 전 실행(선택·기본 꺼짐). 부팅 시 SYSTEM 권한 '시작 시(ONSTART)' 예약 작업으로
    /// LogiFlow 를 <c>--prelogon-pull=&lt;myHost&gt;</c> 로 짧게 띄워, 마우스·키보드를 이 PC 채널로 당긴다.
    /// 설치·삭제는 관리자 권한이 필요하므로 자기 자신을 상승(runas)해 schtasks 를 실행한다.
    ///
    /// 한계(정직 고지):
    ///  - 리시버/동글이 이 PC 에 꽂혀 있고 기기가 다른 채널에 있을 때만 전환 명령이 도달한다.
    ///  - 잠금/로그인 화면에는 커서가 없어 '가장자리 전환'은 불가 — 이 기능은 부팅 시 '이 PC 로 당기기'만 한다.
    ///  - 전환(setHost)은 기존 안전 경로(HostSwitcher) 그대로 — 지원·범위 검증을 거친다.
    /// </summary>
    public static class PreLogon
    {
        public const string TaskName = "LogiFlow-PreLogon";

        /// <summary>'로그온 전에도 실행' 설치 여부 = 윈도우 서비스(LogiFlowLogonSwitch) 존재 여부.
        /// (표준 사용자 권한으로 확인 — 토글 상태 표시용.)</summary>
        public static bool IsInstalled()
        {
            try { return LogiFlow.Service.ServiceControl.Exists(); }
            catch { return false; }
        }

        /// <summary>토글 ON — 상승 요청 후 작업 설치. 성공 true, UAC 취소·정책 차단·실패 false.</summary>
        public static bool RequestEnable(int myHost)
        {
            return RunElevated("--install-prelogon=" + myHost);
        }

        /// <summary>토글 OFF — 상승 요청 후 작업 삭제.</summary>
        public static bool RequestDisable()
        {
            return RunElevated("--uninstall-prelogon");
        }

        private static bool RunElevated(string arg)
        {
            try
            {
                var psi = new ProcessStartInfo(Application.ExecutablePath, arg)
                {
                    UseShellExecute = true,
                    Verb = "runas",   // UAC 상승
                };
                using (var p = Process.Start(psi))
                {
                    if (p == null) return false;
                    p.WaitForExit(20000);
                    return p.ExitCode == 0;
                }
            }
            catch (Win32Exception)
            {
                return false;   // 사용자가 UAC 취소
            }
            catch
            {
                return false;
            }
        }

        // ---- 상승된 인스턴스에서 실행(Program.Main 의 인자 분기에서 호출) ----

        public static int DoInstall(int myHost)
        {
            // 중요: 로그인 전(로그온 화면)엔 사용자 %LocalAppData% 가 안 올라와 SYSTEM 이 그 경로 exe 를
            // 못 찾는다 → exe·의존 파일을 머신 공용 ProgramData 로 복사해 거기서 실행(서비스 binPath 가 이 경로).
            string exe = CopyWorkerToMachine();

            // 구버전 스케줄 작업이 남아 있으면 제거(서비스로 일원화 — 이중 투입 방지).
            try { RunSchtasks("/Delete /TN \"" + TaskName + "\" /F"); } catch { }

            // 윈도우 서비스 설치(LocalSystem·자동시작) — 콜드부팅 포함 모든 부팅에서 로그인 전 상주.
            int rc = LogiFlow.Service.ServiceControl.Install(exe);
            LogLine("서비스 설치(LogiFlowLogonSwitch, 자동시작) rc=" + rc + " exe=" + exe);
            return rc;
        }

        /// <summary>진단용 — schtasks 결과·오류를 로그온 워커와 같은 로그 파일에 남긴다([로그 열기]로 열람).</summary>
        private static void LogLine(string s)
        {
            try
            {
                string p = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "LogiFlow", "desktop-probe.log");
                Directory.CreateDirectory(Path.GetDirectoryName(p));
                File.AppendAllText(p, DateTime.Now.ToString("HH:mm:ss.fff") + "  " + s + "\r\n");
            }
            catch { }
        }

        /// <summary>부팅(BootTrigger)+로그온(LogonTrigger) 2중 트리거 SYSTEM 작업 XML.</summary>
        private static string BuildTaskXml(string exe)
        {
            string cmd = exe.Replace("&", "&amp;").Replace("<", "&lt;").Replace(">", "&gt;");
            var sb = new StringBuilder();
            sb.Append("<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n");
            sb.Append("<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\r\n");
            sb.Append("  <RegistrationInfo><Description>LogiFlow 로그온 화면 전환 워커</Description></RegistrationInfo>\r\n");
            sb.Append("  <Triggers>\r\n");
            sb.Append("    <BootTrigger><Enabled>true</Enabled></BootTrigger>\r\n");
            sb.Append("    <LogonTrigger><Enabled>true</Enabled></LogonTrigger>\r\n");
            sb.Append("  </Triggers>\r\n");
            sb.Append("  <Principals><Principal id=\"Author\"><UserId>S-1-5-18</UserId><RunLevel>HighestAvailable</RunLevel></Principal></Principals>\r\n");
            sb.Append("  <Settings>\r\n");
            sb.Append("    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>\r\n");
            sb.Append("    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>\r\n");
            sb.Append("    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>\r\n");
            sb.Append("    <AllowHardTerminate>true</AllowHardTerminate>\r\n");
            sb.Append("    <StartWhenAvailable>true</StartWhenAvailable>\r\n");
            sb.Append("    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>\r\n");
            sb.Append("    <IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings>\r\n");
            sb.Append("    <AllowStartOnDemand>true</AllowStartOnDemand>\r\n");
            sb.Append("    <Enabled>true</Enabled>\r\n");
            sb.Append("    <Hidden>false</Hidden>\r\n");
            sb.Append("    <RunOnlyIfIdle>false</RunOnlyIfIdle>\r\n");
            sb.Append("    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>\r\n");
            sb.Append("    <Priority>5</Priority>\r\n");
            sb.Append("  </Settings>\r\n");
            sb.Append("  <Actions Context=\"Author\">\r\n");
            sb.Append("    <Exec><Command>").Append(cmd).Append("</Command><Arguments>--logon-switch-launch</Arguments></Exec>\r\n");
            sb.Append("  </Actions>\r\n");
            sb.Append("</Task>\r\n");
            return sb.ToString();
        }

        /// <summary>워커 exe·의존 파일을 머신 공용(ProgramData)으로 복사하고 그 exe 경로를 돌려준다.
        /// 실패하면 원본(사용자 경로) 그대로 사용.</summary>
        private static string CopyWorkerToMachine()
        {
            string src = Application.ExecutablePath;
            try
            {
                string srcDir = Path.GetDirectoryName(src);
                string dstDir = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "LogiFlow", "worker");
                Directory.CreateDirectory(dstDir);
                foreach (string f in Directory.GetFiles(srcDir))
                {
                    string ext = Path.GetExtension(f).ToLowerInvariant();
                    if (ext == ".exe" || ext == ".dll" || ext == ".config")
                    {
                        try { File.Copy(f, Path.Combine(dstDir, Path.GetFileName(f)), true); } catch { }
                    }
                }
                string dstExe = Path.Combine(dstDir, Path.GetFileName(src));
                if (File.Exists(dstExe)) return dstExe;
            }
            catch { }
            return src;
        }

        public static int DoUninstall()
        {
            // 구버전 스케줄 작업도 함께 제거(있으면).
            try { RunSchtasks("/Delete /TN \"" + TaskName + "\" /F"); } catch { }
            int rc = LogiFlow.Service.ServiceControl.Uninstall();
            LogLine("서비스 제거 rc=" + rc);
            return rc;
        }

        private static int RunSchtasks(string args)
        {
            try
            {
                var psi = new ProcessStartInfo("schtasks.exe", args)
                {
                    UseShellExecute = false,
                    CreateNoWindow = true,
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                };
                using (var p = Process.Start(psi))
                {
                    if (p == null) return 1;
                    string so = p.StandardOutput.ReadToEnd();
                    string se = p.StandardError.ReadToEnd();
                    if (!p.WaitForExit(15000)) { try { p.Kill(); } catch { } return 1; }
                    if (p.ExitCode != 0)
                        LogLine("schtasks 실패(" + p.ExitCode + "): " + args + " | "
                            + (se + " " + so).Replace("\r", " ").Replace("\n", " ").Trim());
                    return p.ExitCode;
                }
            }
            catch (Exception ex) { LogLine("schtasks 예외: " + ex.Message + " | args=" + args); return 1; }
        }
    }

    /// <summary>
    /// 로그온 전(SYSTEM·세션0) 짧게 떠서 '이 PC 로 당기기'만 수행하고 종료하는 헤드리스 경로.
    /// UI·트레이를 띄우지 않는다. 쓰기는 HostSwitcher 안전 경로만 사용한다.
    /// </summary>
    public static class PreLogonRunner
    {
        public static void RunPull(int myHost)
        {
            string baseDir = AppDomain.CurrentDomain.BaseDirectory;
            var log = new DiagLogger(baseDir) { Enabled = true };
            try
            {
                log.Log(string.Format("=== 로그온 전 당기기 시작 (myHost 채널 {0}) ===", myHost + 1));
                if (myHost < 0)
                {
                    log.Log("myHost 미지정 — 중단");
                    return;
                }
                List<ScanResult> results = new DeviceScanner(log).Scan();
                var switchable = new List<ScanResult>();
                foreach (var r in results)
                {
                    if (r.ChangeHostSupported && r.HostCount > 0) switchable.Add(r);
                }
                int sent = HostSwitcher.SwitchAll(switchable, myHost, log);
                log.Log(string.Format("로그온 전 당기기 전송: {0}개", sent));
            }
            catch (Exception ex)
            {
                log.LogError("로그온 전 당기기 오류: " + ex);
            }
        }
    }
}
