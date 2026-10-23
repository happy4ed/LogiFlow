using System;
using System.Diagnostics;
using System.Windows.Forms;
using Microsoft.Win32;

namespace LogiFlow.Config
{
    /// <summary>
    /// 로그인 시 트레이 자동 시작. 두 경로를 함께 건다(사용자 범위 · 관리자 권한 불필요):
    ///  ① 로그온 예약 작업(LogiFlow-Tray, ONLOGON) — 윈도우 Run 지연 없이 로그인 직후 바로 뜬다(주).
    ///  ② HKCU\...\Run — 예약 작업이 정책으로 막힌 환경 대비 보조.
    /// 트레이는 단일 인스턴스 뮤텍스를 가지므로 둘 다 떠도 트레이는 하나만 뜬다.
    /// </summary>
    public static class AutoStart
    {
        private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
        private const string ValueName = "LogiFlow";
        private const string TaskName = "LogiFlow-Tray";

        public static bool IsEnabled()
        {
            try
            {
                using (RegistryKey key = Registry.CurrentUser.OpenSubKey(RunKey, false))
                {
                    if (key != null && key.GetValue(ValueName) != null) return true;
                }
            }
            catch { }
            return false;
        }

        public static void Apply(bool enable)
        {
            string exe = Application.ExecutablePath;

            // ① HKCU Run (보조)
            try
            {
                using (RegistryKey key = Registry.CurrentUser.OpenSubKey(RunKey, true)
                                         ?? Registry.CurrentUser.CreateSubKey(RunKey))
                {
                    if (key != null)
                    {
                        if (enable) key.SetValue(ValueName, "\"" + exe + "\"");
                        else if (key.GetValue(ValueName) != null) key.DeleteValue(ValueName, false);
                    }
                }
            }
            catch { /* 정책 제한 등 무시 */ }

            // ② 로그온 예약 작업(주) — 로그인 직후 지연 없이 트레이를 띄운다.
            try
            {
                if (enable)
                {
                    // 현재 사용자 계정으로, 로그온 시, 비상승(일반 권한)으로 실행. 따옴표는 \" 로 이스케이프.
                    string tr = "\\\"" + exe + "\\\"";
                    RunSchtasks("/Create /TN \"" + TaskName + "\" /TR \"" + tr + "\" /SC ONLOGON /RL LIMITED /F");
                }
                else
                {
                    RunSchtasks("/Delete /TN \"" + TaskName + "\" /F");
                }
            }
            catch { /* 예약 작업 실패 시 HKCU Run 으로 대체됨 */ }
        }

        private static void RunSchtasks(string args)
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
                    if (p == null) return;
                    p.StandardOutput.ReadToEnd();
                    p.StandardError.ReadToEnd();
                    if (!p.WaitForExit(8000)) { try { p.Kill(); } catch { } }
                }
            }
            catch { }
        }
    }
}
