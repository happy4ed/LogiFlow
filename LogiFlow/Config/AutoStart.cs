using System;
using System.Windows.Forms;
using Microsoft.Win32;

namespace LogiFlow.Config
{
    /// <summary>로그인 시 자동 시작(HKCU\...\Run). 사용자 범위라 관리자 권한 불필요.</summary>
    public static class AutoStart
    {
        private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
        private const string ValueName = "LogiFlow";

        public static bool IsEnabled()
        {
            try
            {
                using (RegistryKey key = Registry.CurrentUser.OpenSubKey(RunKey, false))
                {
                    if (key == null) return false;
                    object v = key.GetValue(ValueName);
                    return v != null;
                }
            }
            catch
            {
                return false;
            }
        }

        public static void Apply(bool enable)
        {
            try
            {
                using (RegistryKey key = Registry.CurrentUser.OpenSubKey(RunKey, true)
                                         ?? Registry.CurrentUser.CreateSubKey(RunKey))
                {
                    if (key == null) return;
                    if (enable)
                    {
                        string exe = Application.ExecutablePath;
                        key.SetValue(ValueName, "\"" + exe + "\"");
                    }
                    else
                    {
                        if (key.GetValue(ValueName) != null)
                        {
                            key.DeleteValue(ValueName, false);
                        }
                    }
                }
            }
            catch
            {
                // 레지스트리 접근 실패는 무시(정책 제한 등).
            }
        }
    }
}
