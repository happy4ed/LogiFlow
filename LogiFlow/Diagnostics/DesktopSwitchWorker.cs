using System;
using System.Collections.Generic;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Handoff;
using LogiFlow.Ui;

namespace LogiFlow.Diagnostics
{
    /// <summary>
    /// B안(LogiFlow+) — Default(바탕화면) 데스크탑 전용 raw-input 엣지 전환 워커.
    ///
    /// 서비스가 이 워커를 winsta0\default 에 스폰하므로(= 창이 바탕화면 데스크탑에 바로 생성됨)
    /// SetThreadDesktop 없이 STA Application.Run 메시지 펌프를 돌릴 수 있고, 트레이가 쓰던
    /// EdgeWatcher(WM_INPUT raw 마우스)를 그대로 재사용한다. 그래서 화면 끝에 박힌 커서의
    /// '의도적 바깥 밀기'만 전환으로 잡는다 — 폴링 dwell 로 커서를 가장자리에 두기만 해도
    /// 전환되던 오작동이 없다. (로그온 화면은 커서가 클램프돼 밀기를 못 읽으므로 거기선 폴링
    /// 워커 --logon-switch 를 쓴다. 이 워커는 바탕화면 전용.)
    ///
    /// 전환 쓰기는 여전히 HostSwitcher 한 경로뿐(안전 불변식 유지). 콘솔 세션이 바뀌면
    /// self-exit → supervisor 가 상황에 맞는 워커로 교체한다.
    /// </summary>
    public static class DesktopSwitchWorker
    {
        public static void Run()
        {
            var log = new DiagLogger(AppDomain.CurrentDomain.BaseDirectory) { Enabled = true };
            Action<string> say = s => { try { log.Log(s); } catch { } };
            say("==================================================");
            say("desktop-switch(B) 워커 시작 — Default 데스크탑 raw 엣지 전환");

            int dwellMs, myHost; bool debug;
            var edges = LogonEdges.Read(out dwellMs, out myHost, out debug);

            var cfg = new AppConfig { DwellMs = dwellMs };
            foreach (var e in edges)
                cfg.Attachments.Add(new EdgeAttachment { Channel = e.Channel, MonitorDeviceName = e.Device, Edge = (HandoffEdge)e.Edge });
            say(string.Format("엣지 {0}개 · dwell={1}ms", cfg.Attachments.Count, dwellMs));

            // setHost 는 별도 스레드에서(UI 스레드가 막히면 glow 가 멈춤). 진행 중 표시 + 마지막 전환 시각은
            // 백그라운드 스캐너도 본다 — 스캔의 HID++ 핑이 setHost 와 겹치면 리시버가 명령을 놓칠 수 있다.
            int switching = 0;
            long lastSwitchMs = 0;

            // 전환 대상 기기 — 초기 스캔 + 백그라운드 갱신(최신 목록에서 즉시 전송).
            // 기기가 있으면 60초마다만 갱신(전체 스캔은 핑 타임아웃으로 수 초 걸려 거의 상시 리시버를 점유했다),
            // 전환 중·전환 직후 10초는 건너뛴다. 기기가 없을 때만 1.5초마다 재탐색.
            var scanLock = new object();
            var box = new List<ScanResult>[] { ScanSwitchable() };
            say("전환가능 기기 " + box[0].Count + "개");
            var scanner = new Thread(() =>
            {
                long last = Environment.TickCount;
                while (true)
                {
                    Thread.Sleep(300);
                    int have; lock (scanLock) have = box[0].Count;
                    long t = Environment.TickCount;
                    if (have > 0 ? (t - last < 60000) : (t - last < 1500)) continue;
                    if (Interlocked.CompareExchange(ref switching, 0, 0) != 0) continue;
                    if (t - Interlocked.Read(ref lastSwitchMs) < 10000) continue;
                    last = t;
                    var list = ScanSwitchable();
                    if (list.Count > 0) { lock (scanLock) box[0] = list; }
                }
            })
            { IsBackground = true };
            try { scanner.SetApartmentState(ApartmentState.MTA); }
            catch { }
            scanner.Start();

            // glow 오버레이도 이 워커가 바탕화면 데스크탑에 그린다(트레이 감시를 끈 자리 대체).
            // 최고점은 setHost 가 실제로 나간 순간(Delivered) — 전환 성공 시점과 맞춘다.
            var glow = new GlowController();
            var ui = SynchronizationContext.Current ?? new WindowsFormsSynchronizationContext();
            EdgeWatcher watcher = null;

            var hlog = new DiagLogger(AppDomain.CurrentDomain.BaseDirectory) { Enabled = true };
            Action<int> onTrigger = channel =>
            {
                List<ScanResult> devs;
                lock (scanLock) devs = box[0];
                say(string.Format("[바탕화면 전환] → 채널 {0} · 기기 {1}개", channel + 1, devs.Count));
                if (devs.Count == 0) { say("  전환가능 기기 0개 — 건너뜀"); glow.Failed(); return; }
                if (Interlocked.CompareExchange(ref switching, 1, 0) != 0) { say("  이전 전환 진행 중 — 건너뜀"); glow.Failed(); return; }
                Interlocked.Exchange(ref lastSwitchMs, Environment.TickCount);
                var t = new Thread(() =>
                {
                    int ok = 0;
                    try
                    {
                        ok = HostSwitcher.SwitchAllBurst(devs, channel, hlog, say, 3, glow.Delivered);
                        say("  setHost 전환 " + ok + "/" + devs.Count + "개");
                    }
                    catch (Exception ex) { say("  전환 오류: " + ex.Message); }
                    finally
                    {
                        Interlocked.Exchange(ref lastSwitchMs, Environment.TickCount);
                        Interlocked.Exchange(ref switching, 0);
                    }
                    if (ok == 0)
                    {
                        // 하나도 못 넘겼으면 glow 를 흐리고, 변을 떠나지 않고 계속 밀어도 재시도되게 재무장.
                        glow.Failed();
                        ui.Post(_ => { if (watcher != null) watcher.Rearm(); }, null);
                    }
                })
                { IsBackground = true };
                try { t.SetApartmentState(ApartmentState.MTA); }
                catch { }
                t.Start();
            };

            watcher = new EdgeWatcher(log, onTrigger, glow.Update);
            watcher.Configure(cfg);
            watcher.Start();

            // self-exit: 콘솔 세션이 바뀌면(로그아웃/사용자 전환) 종료 → supervisor 가 교체.
            uint mySession = unchecked((uint)System.Diagnostics.Process.GetCurrentProcess().SessionId);
            int off = 0;
            var exitTimer = new System.Windows.Forms.Timer { Interval = 1000 };
            exitTimer.Tick += (s, e) =>
            {
                try
                {
                    uint cur = DesktopProbe.ActiveConsoleSession();
                    if (cur != 0xFFFFFFFF && cur != mySession)
                    {
                        if (++off >= 3) { say("콘솔 세션 변경 → 종료"); Application.ExitThread(); }
                    }
                    else off = 0;
                }
                catch { }
            };
            exitTimer.Start();

            Application.Run(new ApplicationContext());
            try { watcher.Stop(); }
            catch { }
            try { glow.Dispose(); }
            catch { }
        }

        private static List<ScanResult> ScanSwitchable()
        {
            var list = new List<ScanResult>();
            try
            {
                var rr = new DeviceScanner(new DiagLogger(AppDomain.CurrentDomain.BaseDirectory) { Enabled = false }).Scan();
                foreach (var r in rr) if (r.ChangeHostSupported && r.HostCount > 0) list.Add(r);
            }
            catch { }
            return list;
        }
    }
}
