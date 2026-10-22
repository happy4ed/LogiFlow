using System;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Net;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Diagnostics;

namespace LogiFlow.Sharing
{
    /// <summary>
    /// 두 PC 간 클립보드 공유(텍스트·이미지). 같은 서브넷이 아니어도 상대 IP:포트로 직접 TCP 연결한다.
    /// 암호문구(passphrase)에서 유도한 키로 AES-CBC 암호화 + HMAC-SHA256 인증(엿보기·위조 방지).
    /// 클립보드가 바뀌면 상대에게 보내고, 받으면 로컬 클립보드에 적용한다(해시 가드로 에코 루프 방지).
    /// 클립보드 읽기/쓰기는 UI(STA) 스레드에서, 네트워크는 백그라운드에서. Start/Stop 은 UI 스레드에서 호출.
    /// </summary>
    internal sealed class ClipboardSync : IDisposable
    {
        public const int WM_CLIPBOARDUPDATE = 0x031D;
        private const int MaxBytes = 8 * 1024 * 1024; // 8MB 상한
        private static readonly byte[] Salt = Encoding.ASCII.GetBytes("LogiFlow-Clip-v1");

        [DllImport("user32.dll", SetLastError = true)] private static extern bool AddClipboardFormatListener(IntPtr hwnd);
        [DllImport("user32.dll", SetLastError = true)] private static extern bool RemoveClipboardFormatListener(IntPtr hwnd);

        private readonly Control _ui;
        private readonly DiagLogger _log;
        private MsgWindow _win;
        private TcpListener _listener;
        private Thread _acceptThread;
        private volatile bool _running;

        private string _peerHost;
        private int _port;
        private byte[] _aesKey, _macKey;
        private string _lastHash = "";
        private readonly object _gate = new object();

        /// <summary>수신 적용 시 UI 스레드에서 호출(설명 문자열) — 트레이 알림 등.</summary>
        public Action<string> OnReceived;

        public ClipboardSync(Control ui, DiagLogger log) { _ui = ui; _log = log; }

        public bool Running { get { return _running; } }

        /// <summary>설정 적용(UI 스레드에서 호출). 기존 것 멈추고 새로 시작.</summary>
        public void Apply(AppConfig cfg)
        {
            Stop();
            if (cfg == null || !cfg.ClipEnabled) return;
            if (string.IsNullOrEmpty(cfg.ClipSecret)) { _log.Log("클립보드 공유: 암호문구 없음 — 비활성"); return; }
            if (string.IsNullOrEmpty(cfg.ClipPeerHost)) { _log.Log("클립보드 공유: 상대 주소 없음 — 비활성"); return; }
            _peerHost = cfg.ClipPeerHost.Trim();
            _port = cfg.ClipPort;
            DeriveKeys(cfg.ClipSecret);
            Start();
        }

        private void DeriveKeys(string pass)
        {
            using (var kdf = new Rfc2898DeriveBytes(pass, Salt, 20000))
            {
                _aesKey = kdf.GetBytes(32);
                _macKey = kdf.GetBytes(32);
            }
        }

        private void Start()
        {
            try
            {
                _win = new MsgWindow(OnClipboardUpdate);
                AddClipboardFormatListener(_win.Handle);
                _running = true;
                _listener = new TcpListener(IPAddress.Any, _port);
                _listener.Start();
                _acceptThread = new Thread(AcceptLoop) { IsBackground = true, Name = "LogiFlow.Clip.Accept" };
                _acceptThread.Start();
                _log.Log(string.Format("클립보드 공유 시작: 수신 포트 {0} · 상대 {1}:{0}", _port, _peerHost));
            }
            catch (Exception ex)
            {
                _log.LogError("클립보드 공유 시작 실패(포트 사용중/방화벽?): " + ex.Message);
                _running = false;
            }
        }

        public void Stop()
        {
            _running = false;
            try { if (_win != null && _win.Handle != IntPtr.Zero) RemoveClipboardFormatListener(_win.Handle); } catch { }
            try { if (_listener != null) _listener.Stop(); } catch { }
            try { if (_win != null) _win.DestroyHandle(); } catch { }
            _win = null; _listener = null; _acceptThread = null;
        }

        public void Dispose() { Stop(); }

        /// <summary>상대에게 테스트 텍스트를 보낸다(종단 확인). 상대 PC 에서 Ctrl+V 로 확인.</summary>
        public string SendTestNow()
        {
            if (!_running || _aesKey == null) return "먼저 '사용'을 켜고 상대 주소·암호문구를 설정하세요.";
            try
            {
                string msg = "LogiFlow 클립보드 연결 테스트 ✓ " + DateTime.Now.ToString("HH:mm:ss");
                byte[] tb = Encoding.UTF8.GetBytes(msg);
                byte[] payload = new byte[tb.Length + 1]; payload[0] = 0; Array.Copy(tb, 0, payload, 1, tb.Length);
                byte[] frame = Encrypt(payload);
                using (var cli = new TcpClient())
                {
                    var ar = cli.BeginConnect(_peerHost, _port, null, null);
                    if (!ar.AsyncWaitHandle.WaitOne(3000)) return "실패: 상대 연결 시간초과 — 상대 PC 가 켜져 있고 방화벽 허용됐는지 확인.";
                    cli.EndConnect(ar);
                    using (var ns = cli.GetStream())
                    {
                        ns.Write(BitConverter.GetBytes(frame.Length), 0, 4);
                        ns.Write(frame, 0, frame.Length); ns.Flush();
                    }
                }
                return "보냄 OK — 상대 PC 에서 붙여넣기(Ctrl+V) 해서\r\n\"" + msg + "\"\r\n가 나오면 양방향 성공입니다.";
            }
            catch (Exception ex) { return "실패: " + ex.Message; }
        }

        // ---- 진단 보조(설정 화면에서 호출) ----
        /// <summary>이 PC 의 IPv4 주소들 + 포트(상대 PC 에 입력할 값).</summary>
        public static string LocalInfo(int port)
        {
            try
            {
                var sb = new StringBuilder();
                foreach (var ip in Dns.GetHostAddresses(Dns.GetHostName()))
                {
                    if (ip.AddressFamily == AddressFamily.InterNetwork)
                    {
                        if (sb.Length > 0) sb.Append(", ");
                        sb.Append(ip.ToString());
                    }
                }
                return (sb.Length > 0 ? sb.ToString() : "?") + " : " + port;
            }
            catch { return "? : " + port; }
        }

        /// <summary>상대 PC 에 TCP 연결 시도(도달성 확인). 성공/실패 사유 문자열.</summary>
        public static string TestConnect(string host, int port)
        {
            if (string.IsNullOrEmpty(host)) return "상대 주소가 비어 있습니다.";
            try
            {
                using (var cli = new TcpClient())
                {
                    var ar = cli.BeginConnect(host.Trim(), port, null, null);
                    if (!ar.AsyncWaitHandle.WaitOne(3000)) return "실패: 연결 시간초과 — 방화벽/주소/포트 확인.";
                    cli.EndConnect(ar);
                    return "연결 OK — 상대 " + host.Trim() + ":" + port + " 에 닿음.";
                }
            }
            catch (Exception ex) { return "실패: " + ex.Message; }
        }

        // ---- 송신 (UI 스레드에서 호출됨) ----
        private void OnClipboardUpdate()
        {
            byte[] payload = null;
            try
            {
                if (Clipboard.ContainsText())
                {
                    byte[] tb = Encoding.UTF8.GetBytes(Clipboard.GetText() ?? "");
                    payload = new byte[tb.Length + 1]; payload[0] = 0; Array.Copy(tb, 0, payload, 1, tb.Length);
                }
                else if (Clipboard.ContainsImage())
                {
                    using (Image img = Clipboard.GetImage())
                    using (var ms = new MemoryStream())
                    {
                        if (img != null)
                        {
                            img.Save(ms, ImageFormat.Png);
                            byte[] ib = ms.ToArray();
                            if (ib.Length <= MaxBytes)
                            {
                                payload = new byte[ib.Length + 1]; payload[0] = 1; Array.Copy(ib, 0, payload, 1, ib.Length);
                            }
                            else _log.Log("클립보드 이미지 8MB 초과 — 전송 안 함");
                        }
                    }
                }
            }
            catch (Exception ex) { _log.LogError("클립보드 읽기 실패: " + ex.Message); return; }
            if (payload == null) return;

            string hash = Hash(payload);
            lock (_gate) { if (hash == _lastHash) return; _lastHash = hash; }

            byte[] frame;
            try { frame = Encrypt(payload); } catch (Exception ex) { _log.LogError("암호화 실패: " + ex.Message); return; }
            var t = new Thread(() => SendFrame(frame)) { IsBackground = true, Name = "LogiFlow.Clip.Send" };
            t.Start();
        }

        private void SendFrame(byte[] frame)
        {
            try
            {
                using (var cli = new TcpClient())
                {
                    var ar = cli.BeginConnect(_peerHost, _port, null, null);
                    if (!ar.AsyncWaitHandle.WaitOne(3000)) { _log.LogError("클립보드 전송: 상대 연결 시간초과 " + _peerHost); return; }
                    cli.EndConnect(ar);
                    using (var ns = cli.GetStream())
                    {
                        ns.Write(BitConverter.GetBytes(frame.Length), 0, 4);
                        ns.Write(frame, 0, frame.Length);
                        ns.Flush();
                    }
                }
            }
            catch (Exception ex) { _log.LogError("클립보드 전송 실패(" + _peerHost + "): " + ex.Message); }
        }

        // ---- 수신 (백그라운드) ----
        private void AcceptLoop()
        {
            while (_running)
            {
                TcpClient cli = null;
                try { cli = _listener.AcceptTcpClient(); }
                catch { break; }  // listener.Stop()
                try
                {
                    using (cli)
                    using (var ns = cli.GetStream())
                    {
                        cli.ReceiveTimeout = 5000;
                        byte[] lenb = ReadExact(ns, 4); if (lenb == null) continue;
                        int len = BitConverter.ToInt32(lenb, 0);
                        if (len <= 0 || len > MaxBytes + 4096) continue;
                        byte[] frame = ReadExact(ns, len); if (frame == null) continue;
                        byte[] payload = Decrypt(frame);
                        if (payload != null && payload.Length >= 1) ApplyPayload(payload);
                    }
                }
                catch (Exception ex) { _log.LogError("클립보드 수신 처리 실패: " + ex.Message); }
            }
        }

        private void ApplyPayload(byte[] payload)
        {
            string hash = Hash(payload);
            try
            {
                if (_ui.IsDisposed || !_ui.IsHandleCreated) return;
                _ui.BeginInvoke((Action)(() =>
                {
                    try
                    {
                        lock (_gate) { _lastHash = hash; }  // 우리가 세팅 → 뒤따르는 clipboard-update 무시(에코 방지)
                        string desc;
                        if (payload[0] == 0)
                        {
                            string t = Encoding.UTF8.GetString(payload, 1, payload.Length - 1);
                            if (t.Length == 0) Clipboard.Clear(); else Clipboard.SetText(t);
                            desc = "텍스트 " + t.Length + "자";
                            _log.Log("클립보드 수신 적용(" + desc + ")");
                        }
                        else
                        {
                            using (var ms = new MemoryStream(payload, 1, payload.Length - 1))
                            using (Image img = Image.FromStream(ms))
                            {
                                Clipboard.SetImage(img);
                            }
                            desc = "이미지";
                            _log.Log("클립보드 수신 적용(이미지)");
                        }
                        try { if (OnReceived != null) OnReceived(desc); } catch { }
                    }
                    catch (Exception ex) { _log.LogError("클립보드 적용 실패: " + ex.Message); }
                }));
            }
            catch { }
        }

        // ---- 암호화: IV(16) + AES-CBC(ct) + HMAC-SHA256(32) ----
        private byte[] Encrypt(byte[] plain)
        {
            using (var aes = new AesManaged { Key = _aesKey, Mode = CipherMode.CBC, Padding = PaddingMode.PKCS7 })
            {
                aes.GenerateIV();
                byte[] iv = aes.IV, ct;
                using (var enc = aes.CreateEncryptor()) ct = enc.TransformFinalBlock(plain, 0, plain.Length);
                byte[] body = new byte[iv.Length + ct.Length];
                Array.Copy(iv, 0, body, 0, iv.Length); Array.Copy(ct, 0, body, iv.Length, ct.Length);
                using (var h = new HMACSHA256(_macKey))
                {
                    byte[] mac = h.ComputeHash(body);
                    byte[] frame = new byte[body.Length + mac.Length];
                    Array.Copy(body, 0, frame, 0, body.Length); Array.Copy(mac, 0, frame, body.Length, mac.Length);
                    return frame;
                }
            }
        }

        private byte[] Decrypt(byte[] frame)
        {
            try
            {
                if (frame.Length < 16 + 32) return null;
                int bodyLen = frame.Length - 32;
                byte[] body = new byte[bodyLen]; Array.Copy(frame, 0, body, 0, bodyLen);
                byte[] mac = new byte[32]; Array.Copy(frame, bodyLen, mac, 0, 32);
                using (var h = new HMACSHA256(_macKey))
                {
                    if (!ConstEq(h.ComputeHash(body), mac)) { _log.LogError("클립보드 수신 인증 실패(암호문구 불일치?)"); return null; }
                }
                byte[] iv = new byte[16]; Array.Copy(body, 0, iv, 0, 16);
                byte[] ct = new byte[bodyLen - 16]; Array.Copy(body, 16, ct, 0, ct.Length);
                using (var aes = new AesManaged { Key = _aesKey, Mode = CipherMode.CBC, Padding = PaddingMode.PKCS7, IV = iv })
                using (var dec = aes.CreateDecryptor())
                    return dec.TransformFinalBlock(ct, 0, ct.Length);
            }
            catch { return null; }
        }

        private static bool ConstEq(byte[] a, byte[] b)
        {
            if (a == null || b == null || a.Length != b.Length) return false;
            int d = 0; for (int i = 0; i < a.Length; i++) d |= a[i] ^ b[i];
            return d == 0;
        }

        private static string Hash(byte[] data)
        {
            using (var sha = SHA256.Create()) return BitConverter.ToString(sha.ComputeHash(data));
        }

        private static byte[] ReadExact(NetworkStream ns, int count)
        {
            byte[] buf = new byte[count];
            int off = 0;
            while (off < count)
            {
                int r = ns.Read(buf, off, count - off);
                if (r <= 0) return null;
                off += r;
            }
            return buf;
        }

        private sealed class MsgWindow : NativeWindow
        {
            private readonly Action _onUpdate;
            public MsgWindow(Action onUpdate)
            {
                _onUpdate = onUpdate;
                CreateHandle(new CreateParams());
            }
            protected override void WndProc(ref Message m)
            {
                if (m.Msg == WM_CLIPBOARDUPDATE) { try { _onUpdate(); } catch { } }
                base.WndProc(ref m);
            }
        }
    }

    /// <summary>클립보드 수신 포트의 방화벽 인바운드 허용(관리자 권한 netsh). runas 상승 후 실행.</summary>
    internal static class ClipFirewall
    {
        public const string RuleName = "LogiFlow Clipboard";

        public static bool RequestAllow(int port)
        {
            try
            {
                var psi = new ProcessStartInfo(Application.ExecutablePath, "--allow-firewall=" + port) { UseShellExecute = true, Verb = "runas" };
                using (var p = Process.Start(psi))
                {
                    if (p == null) return false;
                    p.WaitForExit(15000);
                    return p.ExitCode == 0;
                }
            }
            catch (System.ComponentModel.Win32Exception) { return false; }
            catch { return false; }
        }

        public static int DoAllow(int port)
        {
            RunNetsh("advfirewall firewall delete rule name=\"" + RuleName + "\"");
            return RunNetsh("advfirewall firewall add rule name=\"" + RuleName + "\" dir=in action=allow protocol=TCP localport=" + port + " profile=any");
        }

        private static int RunNetsh(string args)
        {
            try
            {
                var psi = new ProcessStartInfo("netsh.exe", args) { UseShellExecute = false, CreateNoWindow = true };
                using (var p = Process.Start(psi))
                {
                    if (p == null) return 1;
                    p.WaitForExit(10000);
                    return p.ExitCode;
                }
            }
            catch { return 1; }
        }
    }
}
