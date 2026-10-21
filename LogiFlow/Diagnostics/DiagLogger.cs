using System;
using System.IO;
using System.Text;

namespace LogiFlow.Diagnostics
{
    /// <summary>
    /// 진단 로그. 보낸 요청/받은 응답 raw 바이트(hex)·타임아웃·오류를
    /// logs/logiflow-diag.log 에 남긴다. serial 등 민감값은 기록하지 않는다.
    /// </summary>
    internal sealed class DiagLogger
    {
        private readonly object _lock = new object();
        private readonly StringBuilder _memory = new StringBuilder();

        public string LogDirectory { get; private set; }
        public string LogPath { get; private set; }

        public DiagLogger(string baseDirectory)
        {
            LogDirectory = Path.Combine(baseDirectory, "logs");
            LogPath = Path.Combine(LogDirectory, "logiflow-diag.log");
        }

        public static string Hex(byte[] data)
        {
            if (data == null)
            {
                return "(null)";
            }
            var sb = new StringBuilder(data.Length * 3);
            for (int i = 0; i < data.Length; i++)
            {
                if (i > 0)
                {
                    sb.Append(' ');
                }
                sb.Append(data[i].ToString("X2"));
            }
            return sb.ToString();
        }

        public void Log(string line)
        {
            string stamped = string.Format("{0:yyyy-MM-dd HH:mm:ss.fff}  {1}",
                DateTime.Now, line);
            lock (_lock)
            {
                _memory.AppendLine(stamped);
                try
                {
                    Directory.CreateDirectory(LogDirectory);
                    File.AppendAllText(LogPath, stamped + Environment.NewLine, Encoding.UTF8);
                }
                catch
                {
                    // 파일 기록 실패는 조용히 무시 — 메모리 사본은 유지.
                }
            }
        }

        public void LogTx(byte deviceIndex, byte[] report)
        {
            Log(string.Format("TX  dev=0x{0:X2}  [{1}]", deviceIndex, Hex(report)));
        }

        public void LogRx(byte[] report)
        {
            Log(string.Format("RX  [{0}]", Hex(report)));
        }

        public void LogTimeout(string context)
        {
            Log("TIMEOUT  " + context);
        }

        public void LogError(string context)
        {
            Log("ERROR  " + context);
        }

        public string Snapshot()
        {
            lock (_lock)
            {
                return _memory.ToString();
            }
        }
    }
}
