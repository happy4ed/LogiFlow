using System;
using System.IO;
using System.Windows.Forms;
using LogiFlow.Diagnostics;

namespace LogiFlow
{
    internal static class Program
    {
        [STAThread]
        private static void Main()
        {
            string baseDir = AppDomain.CurrentDomain.BaseDirectory;
            var logger = new DiagLogger(baseDir);
            logger.Log("LogiFlow 시작");

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            try
            {
                Application.Run(new MainForm(logger));
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
                    // UI 표시 실패는 무시.
                }
            }
            finally
            {
                logger.Log("LogiFlow 종료");
            }
        }
    }
}
