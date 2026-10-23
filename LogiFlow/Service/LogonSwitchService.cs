using System;
using System.ServiceProcess;
using System.Threading;
using LogiFlow.Diagnostics;

namespace LogiFlow.Service
{
    /// <summary>
    /// LocalSystem · 자동시작 윈도우 서비스. 콜드부팅 포함 모든 부팅에서 로그인 전에 뜬다
    /// (스케줄 작업 BootTrigger 가 빠른 시작 등으로 안 뜨던 문제를 서비스로 대체).
    ///
    /// 세션0(서비스)엔 입력 데스크톱이 없으므로, 활성 콘솔 세션의 winlogon 데스크톱에
    /// 워커(--logon-switch)를 CreateProcessAsUser 로 투입한다. 콘솔 세션이 바뀌면(로그인·사용자 전환)
    /// 기존 워커를 종료하고 새 세션에 재투입한다. 전환 쓰기는 여전히 HostSwitcher 한 경로뿐.
    /// </summary>
    public sealed class LogonSwitchService : ServiceBase
    {
        public const string SvcName = "LogiFlowLogonSwitch";

        private Thread _sup;
        private volatile bool _stop;
        private IntPtr _worker = IntPtr.Zero;          // 로그온/잠금 전담(winsta0\winlogon, 폴링)
        private uint _workerSession = 0xFFFFFFFF;
        private IntPtr _deskWorker = IntPtr.Zero;      // 바탕화면 전담(winsta0\default, raw input)
        private uint _deskWorkerSession = 0xFFFFFFFF;

        public LogonSwitchService()
        {
            ServiceName = SvcName;
            CanStop = true;
            CanShutdown = true;
            CanHandleSessionChangeEvent = true;
        }

        protected override void OnStart(string[] args)
        {
            _stop = false;
            _sup = new Thread(Supervise) { IsBackground = true };
            try { _sup.SetApartmentState(ApartmentState.MTA); } catch { }
            _sup.Start();
        }

        protected override void OnStop() { StopSupervisor(); }
        protected override void OnShutdown() { StopSupervisor(); }

        private void StopSupervisor()
        {
            _stop = true;
            try { if (_sup != null) _sup.Join(3000); } catch { }
            KillLogonWorker();
            KillDeskWorker();
        }

        private void Supervise()
        {
            DesktopProbe.ServiceLog("supervisor 시작 (서비스 = 콜드부팅부터 상주)");
            while (!_stop)
            {
                try
                {
                    uint cur = DesktopProbe.ActiveConsoleSession();
                    if (cur != 0xFFFFFFFF)
                    {
                        // 세션당 두 워커를 띄운다: winlogon(로그온/잠금) + default(바탕화면).
                        // 각 워커가 자기 입력 데스크톱일 때만 동작하므로 중복 전환이 없다(바탕화면에선
                        // 폴링 워커가 스스로 OFF, raw 워커만 동작).
                        if (_worker == IntPtr.Zero || !DesktopProbe.ProcessAlive(_worker) || cur != _workerSession)
                        {
                            KillLogonWorker();
                            IntPtr h; uint pid, sess;
                            if (DesktopProbe.SpawnLogonWorker(out h, out pid, out sess)) { _worker = h; _workerSession = sess; }
                        }
                        if (_deskWorker == IntPtr.Zero || !DesktopProbe.ProcessAlive(_deskWorker) || cur != _deskWorkerSession)
                        {
                            KillDeskWorker();
                            IntPtr h; uint pid, sess;
                            if (DesktopProbe.SpawnDesktopWorker(out h, out pid, out sess)) { _deskWorker = h; _deskWorkerSession = sess; }
                        }
                    }
                }
                catch (Exception ex) { DesktopProbe.ServiceLog("supervisor 오류: " + ex.Message); }

                // ~2초 대기(중단 시 빠르게 반응).
                for (int i = 0; i < 20 && !_stop; i++) Thread.Sleep(100);
            }
            KillLogonWorker();
            KillDeskWorker();
            DesktopProbe.ServiceLog("supervisor 종료");
        }

        private void KillLogonWorker()
        {
            if (_worker != IntPtr.Zero)
            {
                DesktopProbe.KillProcess(_worker);
                _worker = IntPtr.Zero;
                _workerSession = 0xFFFFFFFF;
            }
        }

        private void KillDeskWorker()
        {
            if (_deskWorker != IntPtr.Zero)
            {
                DesktopProbe.KillProcess(_deskWorker);
                _deskWorker = IntPtr.Zero;
                _deskWorkerSession = 0xFFFFFFFF;
            }
        }
    }
}
