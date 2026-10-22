using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
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
                // 출력 리디렉션 필수 — 콘솔 없는 앱에서 리디렉션 안 하면 schtasks 가 출력 핸들이 없어
                // 비정상 종료코드를 낼 수 있다(x86 토글 꺼짐 원인). 출력은 작아서 데드락 없음.
                var psi = new ProcessStartInfo("schtasks.exe", "/Query /TN \"" + TaskName + "\"")
                {
                    UseShellExecute = false,
                    CreateNoWindow = true,
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                };
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
            // 못 찾는다 → exe·의존 파일을 머신 공용 ProgramData 로 복사해 거기서 실행한다.
            string exe = CopyWorkerToMachine();
            string inner = "\\\"" + exe + "\\\" --logon-switch-launch";
            string args = "/Create /TN \"" + TaskName + "\" /TR \"" + inner + "\" /SC ONSTART /RU SYSTEM /RL HIGHEST /F";
            int rc = RunSchtasks(args);
            // 재부팅 없이 지금 바로 실행(시험용).
            if (rc == 0) RunSchtasks("/Run /TN \"" + TaskName + "\"");
            return rc;
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
