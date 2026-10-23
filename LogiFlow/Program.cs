using System;
using System.ServiceProcess;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Diagnostics;
using LogiFlow.Service;
using LogiFlow.Sharing;

namespace LogiFlow
{
    internal static class Program
    {
        private static Mutex _singleInstance;

        [STAThread]
        private static void Main(string[] args)
        {
            // 로그온 전 서비스 경로(UI 없이 짧게 수행 후 종료) — 트레이 뮤텍스보다 먼저 분기.
            if (args != null && args.Length > 0)
            {
                string a0 = args[0];
                // 윈도우 서비스로 실행(SCM 이 LocalSystem·세션0 에서 호출) — UI·뮤텍스보다 먼저 분기.
                if (a0.StartsWith("--service", StringComparison.OrdinalIgnoreCase))
                {
                    ServiceBase.Run(new LogonSwitchService());
                    return;
                }
                if (a0.StartsWith("--prelogon-pull", StringComparison.OrdinalIgnoreCase))
                {
                    PreLogonRunner.RunPull(ParseEq(a0, -1));
                    return;
                }
                if (a0.StartsWith("--install-prelogon", StringComparison.OrdinalIgnoreCase))
                {
                    Environment.ExitCode = PreLogon.DoInstall(ParseEq(a0, -1));
                    return;
                }
                if (a0.StartsWith("--uninstall-prelogon", StringComparison.OrdinalIgnoreCase))
                {
                    Environment.ExitCode = PreLogon.DoUninstall();
                    return;
                }
                // 로그온 화면 접근 '가능성 확인' 프로브(읽기 전용 · HID/setHost 없음).
                // launch(세션0 런처)를 probe 보다 먼저 검사 — StartsWith 로 접두 겹침 방지.
                if (a0.StartsWith("--desktop-probe-launch", StringComparison.OrdinalIgnoreCase))
                {
                    DesktopProbe.RunLauncher("--desktop-probe=100");
                    return;
                }
                if (a0.StartsWith("--desktop-probe", StringComparison.OrdinalIgnoreCase))
                {
                    DesktopProbe.RunProbe(ParseEq(a0, 120));
                    return;
                }
                // 로그온/잠금 화면 실제 전환: launch(세션0) → winlogon 데스크톱에 worker 투입.
                if (a0.StartsWith("--logon-switch-launch", StringComparison.OrdinalIgnoreCase))
                {
                    DesktopProbe.RunLauncher("--logon-switch");
                    return;
                }
                if (a0.StartsWith("--logon-switch", StringComparison.OrdinalIgnoreCase))
                {
                    DesktopProbe.RunLogonSwitch();
                    return;
                }
                if (a0.StartsWith("--install-probe", StringComparison.OrdinalIgnoreCase))
                {
                    Environment.ExitCode = ProbeTask.DoInstallRun();
                    return;
                }
                if (a0.StartsWith("--uninstall-probe", StringComparison.OrdinalIgnoreCase))
                {
                    Environment.ExitCode = ProbeTask.DoUninstall();
                    return;
                }
                // 클립보드 수신 포트 방화벽 허용(상승된 인스턴스에서 netsh).
                if (a0.StartsWith("--allow-firewall", StringComparison.OrdinalIgnoreCase))
                {
                    Environment.ExitCode = ClipFirewall.DoAllow(ParseEq(a0, 48737));
                    return;
                }
            }

            bool createdNew;
            _singleInstance = new Mutex(true, "LogiFlow-A2E1C4D6-8B3F-4A91-9C72-1E5D7F0A2B34", out createdNew);
            if (!createdNew)
            {
                // 이미 실행 중(자동시작 + 수동 실행 등) — 중복 트레이 방지.
                _singleInstance.Dispose();
                return;
            }

            string baseDir = AppDomain.CurrentDomain.BaseDirectory;
            var logger = new DiagLogger(baseDir);
            logger.Log("LogiFlow 시작(트레이)");

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            try
            {
                Application.Run(new TrayAppContext(logger));
            }
            catch (Exception ex)
            {
                logger.LogError("치명적 오류: " + ex);
                try
                {
                    MessageBox.Show("LogiFlow 실행 중 오류가 발생했습니다.\r\n로그: " + logger.LogPath
                        + "\r\n\r\n" + ex.Message, "LogiFlow",
                        MessageBoxButtons.OK, MessageBoxIcon.Error);
                }
                catch
                {
                }
            }
            finally
            {
                logger.Log("LogiFlow 종료");
            }
        }

        /// <summary>"--flag=값" 에서 정수 값 파싱. 없으면 def.</summary>
        private static int ParseEq(string arg, int def)
        {
            int eq = arg.IndexOf('=');
            if (eq < 0) return def;
            int v;
            return int.TryParse(arg.Substring(eq + 1), out v) ? v : def;
        }
    }
}
