using System;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Diagnostics;

namespace LogiFlow
{
    internal static class Program
    {
        private static Mutex _singleInstance;

        [STAThread]
        private static void Main()
        {
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
    }
}
