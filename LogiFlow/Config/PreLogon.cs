using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
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

        /// <summary>예약 작업 존재 여부(schtasks /Query 종료코드 0).</summary>
        public static bool IsInstalled()
        {
            try
            {
                // 출력은 리디렉션하지 않는다(파이프 버퍼 데드락 방지) — CreateNoWindow 로 콘솔만 숨기고 종료코드만 본다.
                var psi = new ProcessStartInfo("schtasks.exe", "/Query /TN \"" + TaskName + "\"")
                {
                    UseShellExecute = false,
                    CreateNoWindow = true,
                };
                using (var p = Process.Start(psi))
                {
                    if (p == null) return false;
                    if (!p.WaitForExit(5000)) { try { p.Kill(); } catch { } return false; }
                    return p.ExitCode == 0;
                }
            }
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
            string exe = Application.ExecutablePath;
            // schtasks /TR 에는 따옴표로 감싼 '"exe" --arg' 형태를 넣는다.
            // 세션0 런처 → winlogon 데스크톱에 '로그온 전환 워커' 투입(엣지 설정은 머신 공유 파일에서).
            string inner = "\\\"" + exe + "\\\" --logon-switch-launch";
            string args = "/Create /TN \"" + TaskName + "\" /TR \"" + inner + "\" /SC ONSTART /RU SYSTEM /RL HIGHEST /F";
            return RunSchtasks(args);
        }

        public static int DoUninstall()
        {
            return RunSchtasks("/Delete /TN \"" + TaskName + "\" /F");
        }

        private static int RunSchtasks(string args)
        {
            try
            {
                var psi = new ProcessStartInfo("schtasks.exe", args)
                {
                    UseShellExecute = false,
                    CreateNoWindow = true,
                };
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
