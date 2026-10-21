using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Diagnostics;
using LogiFlow.Ui;

namespace LogiFlow
{
    /// <summary>
    /// 승인 시안(사이드바 내비 + 전환 관리·설정·진단 3화면)의 WinForms 구현.
    /// 창 크기는 페이지와 무관하게 고정(리사이즈·최소크기 허용), 내용이 많으면 스크롤.
    /// </summary>
    internal sealed class MainForm : Form
    {
        private readonly TrayAppContext _tray;
        private AppConfig Config { get { return _tray.Config; } }
        private DiagLogger Log { get { return _tray.Log; } }

        private Panel _sidebar;
        private ThemedScrollPanel _pageHost;
        private NavButton _navSwitch, _navSettings, _navDiag;
        private int _pageIndex;
        private bool _busy;
        private bool _preLogonInstalled;   // 로그온 전 작업 존재 여부(캐시 — 설정 빌드마다 schtasks 안 돌게).

        public MainForm(TrayAppContext tray)
        {
            _tray = tray;
            BuildUi();
            Theme.Changed += OnThemeChanged;
            StartPreLogonStateProbe();
        }

        /// <summary>로그온 전 작업 존재 여부를 백그라운드로 1회 조회해 캐시(설정 탭 지연 방지).</summary>
        private void StartPreLogonStateProbe()
        {
            var t = new Thread(() =>
            {
                bool installed = PreLogon.IsInstalled();
                _preLogonInstalled = installed;   // 다음 설정 빌드에서 반영되도록 먼저 기록.
                try
                {
                    if (!IsDisposed && IsHandleCreated)
                        BeginInvoke((Action)(() => { if (_pageIndex == 1) ShowPage(1); }));
                }
                catch { }
            });
            t.IsBackground = true;
            t.Start();
        }

        private void BuildUi()
        {
            Text = "LogiFlow";
            Icon = AppIcon.Get();
            StartPosition = FormStartPosition.CenterScreen;
            ClientSize = new Size(820, 660);
            MinimumSize = new Size(760, 600);
            Font = UiFont.Get(10f);

            _sidebar = new Panel { Dock = DockStyle.Left, Width = 182, Padding = new Padding(12, 14, 12, 12) };
            BuildSidebar();

            _pageHost = new ThemedScrollPanel { Dock = DockStyle.Fill };
            _pageHost.ViewportWidthChanged += (s, e) => ShowPage(_pageIndex);

            Controls.Add(_pageHost);
            Controls.Add(_sidebar);

            ApplyChrome();
            FormClosing += OnFormClosing;
            UiFont.Changed += OnFontChanged;
            ShowPage(0);
        }

        private void OnFontChanged(object s, EventArgs e)
        {
            DeferRebuild();
        }

        // 테마/폰트 변경은 보통 활성 컨트롤(세그먼트)의 이벤트 중에 발생한다.
        // 그 자리에서 재빌드하면 이벤트를 발생시킨 컨트롤이 dispose 되어 위험하므로 BeginInvoke 로 지연.
        private void DeferRebuild()
        {
            if (IsDisposed || !IsHandleCreated) return;
            try
            {
                BeginInvoke((Action)(() =>
                {
                    if (IsDisposed) return;
                    Font = UiFont.Get(10f);
                    ApplyChrome();
                    BuildSidebar();
                    ShowPage(_pageIndex);
                }));
            }
            catch { }
        }

        private void BuildSidebar()
        {
            DisposeChildren(_sidebar);
            Color sb = Theme.Sidebar;

            var brand = new Panel { Dock = DockStyle.Top, Height = 40, Padding = new Padding(4, 2, 4, 2), BackColor = sb };
            var mark = new IconView(Icons.Mark, 18) { Left = 4, Top = 9, Tint = Theme.Accent };
            var name = new Label { Text = "LogiFlow", Left = 30, Top = 8, AutoSize = true, Font = UiFont.Get(10f, FontStyle.Bold), ForeColor = Theme.Ink };
            brand.Controls.Add(mark);
            brand.Controls.Add(name);

            _navSwitch = MakeNav("전환 관리", Icons.Swap, 0);
            _navSettings = MakeNav("설정", Icons.Gear, 1);
            _navDiag = MakeNav("진단", Icons.Diag, 2);

            var navHost = new Panel { Dock = DockStyle.Top, Height = 3 * 36 + 2 * 4 + 8, Padding = new Padding(0, 8, 0, 0), BackColor = sb };
            _navDiag.Dock = DockStyle.Top; _navDiag.Height = 36;
            _navSettings.Dock = DockStyle.Top; _navSettings.Height = 36;
            _navSwitch.Dock = DockStyle.Top; _navSwitch.Height = 36;
            navHost.Controls.Add(_navDiag);
            navHost.Controls.Add(MakeGap(4));
            navHost.Controls.Add(_navSettings);
            navHost.Controls.Add(MakeGap(4));
            navHost.Controls.Add(_navSwitch);

            var status = new Panel { Dock = DockStyle.Bottom, Height = 32, BackColor = sb };
            var plug = new IconView(Icons.Plug, 16) { Left = 6, Top = 8, Tint = Theme.Muted };
            var dot = new IconView((g, r, c) => { using (var b = new SolidBrush(Theme.Ok)) g.FillEllipse(b, r.X, r.Y + r.Height / 2 - 3.5f, 7, 7); }, 16) { Left = 26, Top = 8 };
            var slabel = new Label { Text = "LogiFlow 실행 중", Left = 42, Top = 7, AutoSize = true, ForeColor = Theme.Muted, Font = UiFont.Get(8.5f) };
            status.Controls.Add(plug);
            status.Controls.Add(dot);
            status.Controls.Add(slabel);

            _sidebar.Controls.Add(navHost);
            _sidebar.Controls.Add(status);
            _sidebar.Controls.Add(brand);

            UpdateNavActive();
        }

        private NavButton MakeNav(string text, IconPaint icon, int index)
        {
            var nav = new NavButton(text, icon) { Font = UiFont.Get(9.5f) };
            nav.Selected += (s, e) => ShowPage(index);
            return nav;
        }

        private static Panel MakeGap(int h)
        {
            return new Panel { Dock = DockStyle.Top, Height = h, BackColor = Theme.Sidebar };
        }

        private void UpdateNavActive()
        {
            if (_navSwitch == null) return;
            _navSwitch.Active = _pageIndex == 0;
            _navSettings.Active = _pageIndex == 1;
            _navDiag.Active = _pageIndex == 2;
        }

        private void ApplyChrome()
        {
            BackColor = Theme.Content;
            if (_sidebar != null) _sidebar.BackColor = Theme.Sidebar;
            if (_pageHost != null) _pageHost.BackColor = Theme.Content;
        }

        private void OnThemeChanged(object s, EventArgs e)
        {
            DeferRebuild();
        }

        private static void DisposeChildren(Control host)
        {
            var old = new List<Control>();
            foreach (Control c in host.Controls) old.Add(c);
            host.Controls.Clear();
            foreach (Control c in old) c.Dispose();
        }

        // 콘텐츠 열 최대 폭 — 창이 넓어도 이 폭 이상 늘어나지 않게 해 캔버스·행이 과하게 벌어지지 않는다.
        private const int ContentMax = 560;

        /// <summary>콘텐츠 열 폭(뷰포트 - 좌우 여백, 상한 ContentMax).</summary>
        private static int Cw(int vw) { return Math.Min(vw - 48, ContentMax); }

        public void ShowPage(int index)
        {
            _pageIndex = index;
            UpdateNavActive();
            int vw = Math.Max(500, _pageHost.ViewportWidth);
            // 레이아웃·그리기 정지로 탭 전환 시 중간 재배치·깜빡임을 없앤다.
            _pageHost.SuspendLayout();
            try
            {
                Panel page;
                switch (index)
                {
                    case 1: page = BuildSettingsPage(vw); break;
                    case 2: page = BuildDiagPage(vw); break;
                    default: page = BuildSwitchPage(vw); break;
                }
                page.Location = new Point(0, 0);
                _pageHost.SetContent(page);
            }
            finally
            {
                _pageHost.ResumeLayout(true);
            }
        }

        // ---- 공통 빌더 ----
        private Label Title(string text)
        {
            return new Label { Text = text, AutoSize = true, Font = UiFont.Get(14f, FontStyle.Bold), ForeColor = Theme.Ink };
        }

        private Label Sub(string text)
        {
            return new Label { Text = text, AutoSize = true, Font = UiFont.Get(9f), ForeColor = Theme.Muted };
        }

        private Label SectionLabel(string text, string small)
        {
            var l = new Label { AutoSize = true, Font = UiFont.Get(9.5f, FontStyle.Bold), ForeColor = Theme.Ink };
            l.Text = small != null ? text + "   " + small : text;
            return l;
        }

        private Label GroupHead(string text)
        {
            return new Label { Text = text.ToUpper(), AutoSize = true, Font = UiFont.Get(8f, FontStyle.Bold), ForeColor = Theme.Muted };
        }

        private Panel Card(int x, int y, int w, int h)
        {
            var p = new ThemedCard { Left = x, Top = y, Width = w, Height = h };
            return p;
        }

        // ---- 전환 관리 ----
        private Panel BuildSwitchPage(int vw)
        {
            var page = new Panel { Width = vw, Padding = new Padding(24, 20, 24, 20), BackColor = Theme.Content };
            int W = Cw(vw);
            int y = 20;

            var title = Title("전환 관리"); title.Location = new Point(24, y); page.Controls.Add(title);
            var sub = Sub("화면 끝으로 밀면 자동 전환 · 떨어뜨리면 수동"); sub.Location = new Point(24, y + 28); page.Controls.Add(sub);
            var refresh = new ThemedButton { Variant = ButtonVariant.Icon, Icon = Icons.Refresh, Width = 32, Height = 30, Left = 24 + W - 32, Top = y }; refresh.Click += (s, e) => _tray.Rescan();
            page.Controls.Add(refresh);
            y += 58;

            // 캔버스는 적정 폭/높이로 고정하고, 모니터가 넘치면 그 영역만 상하좌우 스크롤(AutoScroll 호스트).
            int canvasH = 300;
            var canvasHost = new Panel { Left = 24, Top = y, Width = W, Height = canvasH, BackColor = Theme.CanvasBg, AutoScroll = true };
            var canvas = new MonitorLayoutPanel { Left = 0, Top = 0 };
            int maxHost = _tray.MaxHostCount();
            int current = _tray.CurrentChannel();
            var channels = new List<int>();
            for (int h = 0; h < maxHost; h++) if (h != current) channels.Add(h);
            canvas.SetData(channels, current, Config.Attachments, Config.AliasFor);
            canvas.AttachmentsChanged += (s, e) =>
            {
                Config.Attachments.Clear();
                Config.Attachments.AddRange(canvas.GetAttachments());
                _tray.ApplyConfig();
            };
            canvasHost.Controls.Add(canvas);
            canvasHost.ClientSizeChanged += (s, e) => canvas.RelayoutToHost(canvasHost.ClientSize);
            canvas.RelayoutToHost(new Size(W, canvasH));
            page.Controls.Add(canvasHost);
            y += canvasH + 14;

            // 채널 별칭
            var aliasLbl = SectionLabel("채널 별칭", "엣지에 둘 다른 PC 를 알아보기 쉽게"); aliasLbl.Location = new Point(24, y); page.Controls.Add(aliasLbl);
            y += 24;
            int ax = 24;
            int aliasCount = Math.Max(maxHost, 1);
            for (int ch = 0; ch < aliasCount; ch++)
            {
                int channel = ch;
                var chip = BuildAliasChip(channel);
                chip.Location = new Point(ax, y);
                page.Controls.Add(chip);
                ax += chip.Width + 8;
            }
            y += 42;

            // 등록된 기기
            var devices = _tray.Switchable();
            var devLbl = SectionLabel("등록된 기기", devices.Count + "개"); devLbl.Location = new Point(24, y); page.Controls.Add(devLbl);
            y += 26;
            if (devices.Count == 0)
            {
                var none = new Label { Text = "전환 가능한 기기가 없습니다. [진단]에서 새로 고침 하세요.", AutoSize = true, ForeColor = Theme.Muted, Location = new Point(24, y) };
                page.Controls.Add(none);
                y += 28;
            }
            foreach (var d in devices)
            {
                var row = BuildDeviceRow(d, W);
                row.Location = new Point(24, y);
                page.Controls.Add(row);
                y += row.Height;
            }
            y += 8;

            // footer
            var foot = BuildSwitchFooter(W, canvas);
            foot.Location = new Point(24, y);
            page.Controls.Add(foot);
            y += foot.Height + 20;

            page.Height = y;
            return page;
        }

        private Panel BuildAliasChip(int channel)
        {
            var chip = new ThemedCard { Height = 30, Padding = new Padding(8, 0, 8, 0) };
            var cn = new Label { Text = "채널 " + (channel + 1), AutoSize = true, Font = UiFont.Get(8f, FontStyle.Bold), ForeColor = Theme.Accent, Top = 7, Left = 8 };
            var al = new Label { Text = Config.AliasFor(channel), AutoSize = true, Font = UiFont.Get(8.5f), ForeColor = Theme.Ink, Top = 7 };
            al.Left = cn.Right + 8;
            var pen = new ThemedButton { Variant = ButtonVariant.Icon, Icon = Icons.Pencil, Width = 24, Height = 22, Top = 4 };
            pen.Left = al.Right + 6;
            pen.Click += (s, e) => EditAlias(channel);
            chip.Controls.Add(cn); chip.Controls.Add(al); chip.Controls.Add(pen);
            chip.Width = pen.Right + 8;
            // 칩 클릭도 편집.
            al.Click += (s, e) => EditAlias(channel);
            return chip;
        }

        private void EditAlias(int channel)
        {
            string cur = Config.AliasFor(channel);
            string v = Prompt.Show(this, "채널 별칭", string.Format("채널 {0} 의 별칭", channel + 1), cur);
            if (v == null) return;
            if (v.Length == 0) Config.Aliases.Remove(channel);
            else Config.Aliases[channel] = v;
            _tray.ApplyConfig();
            ShowPage(0);
        }

        private Panel BuildDeviceRow(ScanResult d, int w)
        {
            var row = new Panel { Width = w, Height = 44, BackColor = Theme.Content };
            bool isKeyboard = d.DeviceName != null && d.DeviceName.IndexOf("keyboard", StringComparison.OrdinalIgnoreCase) >= 0
                || (d.DeviceName != null && (d.DeviceName.IndexOf("Mechanical", StringComparison.OrdinalIgnoreCase) >= 0 || d.DeviceName.IndexOf("Keys", StringComparison.OrdinalIgnoreCase) >= 0));
            var icon = new IconView(isKeyboard ? (IconPaint)Icons.Keyboard : Icons.Mouse, 18) { Left = 2, Top = 13, Tint = Theme.Muted };
            var name = new Label { Text = d.DeviceName, AutoSize = true, Font = UiFont.Get(9.5f), ForeColor = Theme.Ink, Left = 30, Top = 13 };

            string chText = d.CurrentHost >= 0 ? Config.AliasFor(d.CurrentHost) : "?";
            var badge = new ThemedBadge { Text = "채널 " + (d.CurrentHost >= 0 ? (d.CurrentHost + 1).ToString() : "?"), Left = w - 220, Top = 10, Height = 24, AutoWidth = true };

            var switchBtn = new ThemedButton { Text = "전환", Width = 64, Height = 26, Left = w - 72, Top = 9 };
            switchBtn.Click += (s, e) => ShowDeviceSwitchMenu(d, switchBtn);

            row.Controls.Add(icon); row.Controls.Add(name); row.Controls.Add(badge); row.Controls.Add(switchBtn);
            var sep = new Panel { Left = 0, Top = 43, Width = w, Height = 1, BackColor = Theme.Divider };
            row.Controls.Add(sep);
            return row;
        }

        private void ShowDeviceSwitchMenu(ScanResult d, Control anchor)
        {
            var menu = new ContextMenuStrip();
            int hosts = d.HostCount > 0 ? d.HostCount : _tray.MaxHostCount();
            for (int h = 0; h < hosts; h++)
            {
                if (h == d.CurrentHost) continue;
                int target = h;
                var item = new ToolStripMenuItem(string.Format("채널 {0} · {1}", h + 1, Config.AliasFor(h)));
                item.Click += (s, e) => DoSwitchOne(d, target);
                menu.Items.Add(item);
            }
            if (menu.Items.Count == 0) menu.Items.Add(new ToolStripMenuItem("전환할 다른 채널 없음") { Enabled = false });
            menu.Show(anchor, new Point(0, anchor.Height));
        }

        private Panel BuildSwitchFooter(int w, MonitorLayoutPanel canvas)
        {
            var foot = new Panel { Width = w, Height = 86, BackColor = Theme.Content };
            var top = new Panel { Left = 0, Top = 0, Width = w, Height = 1, BackColor = Theme.Divider };
            foot.Controls.Add(top);

            var edgeIcon = new IconView(Icons.Edge, 16) { Left = 0, Top = 16, Tint = Theme.Muted };
            var edgeLbl = new Label { Text = "가장자리 자동 전환", AutoSize = true, Font = UiFont.Get(9.5f, FontStyle.Bold), ForeColor = Theme.Ink, Left = 24, Top = 15 };

            var dwellLbl = new Label { Text = "대기", AutoSize = true, ForeColor = Theme.Muted, Font = UiFont.Get(8.5f), Top = 18 };
            var dwellBtn = new ThemedButton { Text = DwellText(Config.DwellMs), Icon = null, Width = 78, Height = 26, Top = 14 };
            dwellBtn.Click += (s, e) => ShowDwellMenu(dwellBtn);
            var edgeToggle = new ThemedToggle { Checked = Config.EdgeSwitchEnabled, Top = 20 };
            edgeToggle.CheckedChanged += (s, e) => { Config.EdgeSwitchEnabled = edgeToggle.Checked; _tray.ApplyConfig(); };

            edgeToggle.Left = w - 40; dwellBtn.Left = edgeToggle.Left - 90; dwellLbl.Left = dwellBtn.Left - 30;
            foot.Controls.Add(edgeIcon); foot.Controls.Add(edgeLbl); foot.Controls.Add(dwellLbl); foot.Controls.Add(dwellBtn); foot.Controls.Add(edgeToggle);

            var guardChk = new ThemedToggle { Checked = Config.DragGuard, Top = 52 };
            var guardLbl = new Label { Text = "드래그 중 전환 안 함", AutoSize = true, ForeColor = Theme.Ink, Font = UiFont.Get(8.5f), Left = 50, Top = 56 };
            guardChk.Left = 0;
            guardChk.CheckedChanged += (s, e) => { Config.DragGuard = guardChk.Checked; _tray.ApplyConfig(); };

            var reset = new ThemedButton { Variant = ButtonVariant.Ghost, Text = "배치 초기화", Width = 90, Height = 26, Top = 54 };
            reset.Left = w - 90;
            reset.Click += (s, e) => { canvas.ClearPlacements(); };
            foot.Controls.Add(guardChk); foot.Controls.Add(guardLbl); foot.Controls.Add(reset);
            return foot;
        }

        private static string DwellText(int ms)
        {
            return (ms / 1000.0).ToString("0.##") + "초";
        }

        private void ShowDwellMenu(Control anchor)
        {
            var menu = new ContextMenuStrip();
            int[] opts = { 200, 330, 500, 800, 1000 };
            foreach (int ms in opts)
            {
                int v = ms;
                var item = new ToolStripMenuItem(DwellText(ms));
                item.Click += (s, e) => { Config.DwellMs = v; _tray.ApplyConfig(); ShowPage(0); };
                menu.Items.Add(item);
            }
            menu.Show(anchor, new Point(0, anchor.Height));
        }

        // ---- 설정 ----
        private Panel BuildSettingsPage(int vw)
        {
            var page = new Panel { Width = vw, BackColor = Theme.Content };
            int W = Cw(vw);
            int y = 20;
            var title = Title("설정"); title.Location = new Point(24, y); page.Controls.Add(title);
            var sub = Sub("실행과 로그를 관리"); sub.Location = new Point(24, y + 28); page.Controls.Add(sub);
            y += 60;

            var g1 = GroupHead("모양"); g1.Location = new Point(24, y); page.Controls.Add(g1); y += 22;
            var themeCard = Card(24, y, W, 2 * 66);

            // 테마
            var seg = new SegmentedControl { Left = W - 246, Top = 17, Width = 224, Height = 34 };
            seg.SetItems(new[] { "시스템", "라이트", "다크" }, (int)Config.Theme);
            seg.SelectedChanged += (s, e) =>
            {
                Config.Theme = (ThemeMode)seg.SelectedIndex;
                Theme.SetMode(Config.Theme);
                _tray.ApplyConfig();
            };
            themeCard.Controls.Add(MakeRowLabel("테마", "시스템 설정을 따르거나 라이트·다크로 고정합니다.", 16));
            themeCard.Controls.Add(seg);
            themeCard.Controls.Add(new Panel { Left = 16, Top = 66, Width = W - 32, Height = 1, BackColor = Theme.Divider });

            // 글자 크기(pt 가산: 작게 -1 · 보통 +1 · 크게 +3)
            var segFont = new SegmentedControl { Left = W - 246, Top = 83, Width = 224, Height = 34 };
            int fontIdx = Config.FontOffset >= 3 ? 2 : (Config.FontOffset <= -1 ? 0 : 1);
            segFont.SetItems(new[] { "작게", "보통", "크게" }, fontIdx);
            segFont.SelectedChanged += (s, e) =>
            {
                int off = segFont.SelectedIndex == 2 ? 3 : (segFont.SelectedIndex == 0 ? -1 : 1);
                Config.FontOffset = off;
                UiFont.SetOffset(off);
                _tray.ApplyConfig();
            };
            themeCard.Controls.Add(MakeRowLabel("글자 크기", "보통이 기본입니다. 작은 글자가 안 보이면 크게로 키우세요.", 82));
            themeCard.Controls.Add(segFont);

            page.Controls.Add(themeCard);
            y += 2 * 66 + 16;

            var g2 = GroupHead("실행"); g2.Location = new Point(24, y); page.Controls.Add(g2); y += 22;
            var runCard = Card(24, y, W, 2 * 60);
            AddSettingRow(runCard, 0, "Windows 시작 시 실행", "로그인하면 트레이에 자동으로 상주합니다.", false,
                Config.AutoStart, (on) => { Config.AutoStart = on; _tray.ApplyConfig(); });
            AddSettingRow(runCard, 60, "로그온 전에도 실행", "로그인 화면에서도 엣지 전환이 되게 하는 기능(보안 데스크톱 접근)은 방식 확정 후 제공합니다. 켜도 지금은 설치되지 않습니다.", true,
                _preLogonInstalled, TogglePreLogon);
            page.Controls.Add(runCard);
            y += 2 * 60 + 16;

            var g3 = GroupHead("전환 · 진단"); g3.Location = new Point(24, y); page.Controls.Add(g3); y += 22;
            var swCard = Card(24, y, W, 2 * 60);
            AddSettingRow(swCard, 0, "가장자리 자동 전환 사용", "모니터 변에 붙인 채널로 커서를 밀면 자동 전환합니다.", false,
                Config.EdgeSwitchEnabled, (on) => { Config.EdgeSwitchEnabled = on; _tray.ApplyConfig(); });
            AddSettingRow(swCard, 60, "진단 로그 남기기", "엣지 판정·전환 트리거를 로그 폴더에 기록합니다.", false,
                Config.DiagLogEnabled, (on) => { Config.DiagLogEnabled = on; Log.Enabled = on; _tray.ApplyConfig(); });
            page.Controls.Add(swCard);
            y += 2 * 60 + 20;

            page.Height = y;
            return page;
        }

        private void TogglePreLogon(bool on)
        {
            if (on)
            {
                // 로그인 화면 엣지 전환은 보안 데스크톱(Winlogon) 접근이 필요 — 방식 확정 전까지 설치하지 않는다(정직).
                MessageBox.Show(this,
                    "로그인 화면에서의 엣지 전환은 윈도우 보안 화면(Winlogon) 접근이 필요해 방식 확정 후 제공합니다.\r\n지금은 아무것도 설치하지 않습니다.",
                    "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Information);
            }
            else
            {
                // 끄기: 이전 판(0.8.0)에서 설치된 부팅 작업이 남아 있으면 확실히 제거한다.
                if (_preLogonInstalled || PreLogon.IsInstalled())
                {
                    bool ok = PreLogon.RequestDisable();
                    MessageBox.Show(this,
                        ok ? "로그온 전 실행(이전 판 설치분)을 제거했습니다."
                           : "제거가 취소되었거나 실패했습니다. 관리자 권한(UAC)이 필요합니다.",
                        "LogiFlow", MessageBoxButtons.OK, ok ? MessageBoxIcon.Information : MessageBoxIcon.Warning);
                }
            }
            Config.PreLogon = false;
            _preLogonInstalled = PreLogon.IsInstalled();
            _tray.ApplyConfig();
            BeginInvoke((Action)(() => ShowPage(1)));
        }

        private Control MakeRowLabel(string lab, string desc, int top)
        {
            var p = new Panel { Left = 16, Top = top, Width = 290, Height = 40, BackColor = Theme.Section };
            var l1 = new Label { Text = lab, AutoSize = true, Font = UiFont.Get(9.5f, FontStyle.Bold), ForeColor = Theme.Ink, Left = 0, Top = 0 };
            var l2 = new Label { Text = desc, AutoSize = false, Font = UiFont.Get(8.5f), ForeColor = Theme.Muted, Left = 0, Top = 19, Width = 288, Height = 32 };
            p.Controls.Add(l1); p.Controls.Add(l2);
            return p;
        }

        private void AddSettingRow(Panel card, int top, string lab, string desc, bool advanced, bool on, Action<bool> onChange)
        {
            var row = new Panel { Left = 0, Top = top, Width = card.Width, Height = 60, BackColor = Theme.Section };
            var l1 = new Label { Text = lab, AutoSize = true, Font = UiFont.Get(9.5f, FontStyle.Bold), ForeColor = Theme.Ink, Left = 16, Top = 12 };
            row.Controls.Add(l1);
            if (advanced)
            {
                var adv = new ThemedBadge { Text = "고급", Left = l1.Right + 8, Top = 11, Height = 18, AutoWidth = true, UseAccent = true };
                row.Controls.Add(adv);
            }
            var l2 = new Label { Text = desc, AutoSize = false, Font = UiFont.Get(8.5f), ForeColor = Theme.Muted, Left = 16, Top = 32, Width = card.Width - 90, Height = 18 };
            row.Controls.Add(l2);
            var tg = new ThemedToggle { Checked = on, Left = card.Width - 56, Top = 19 };
            tg.CheckedChanged += (s, e) => onChange(tg.Checked);
            row.Controls.Add(tg);
            if (top > 0)
            {
                var sep = new Panel { Left = 16, Top = 0, Width = card.Width - 32, Height = 1, BackColor = Theme.Divider };
                row.Controls.Add(sep);
            }
            card.Controls.Add(row);
        }

        // ---- 진단 ----
        private Panel BuildDiagPage(int vw)
        {
            var page = new Panel { Width = vw, BackColor = Theme.Content };
            int W = Cw(vw);
            int y = 20;
            var title = Title("진단"); title.Location = new Point(24, y); page.Controls.Add(title);
            var sub = Sub("기기별 채널·전환 인덱스 확인"); sub.Location = new Point(24, y + 28); page.Controls.Add(sub);

            bool scanning = _tray.IsScanning;
            var logBtn = new ThemedButton { Text = "로그 폴더 열기", Icon = Icons.Folder, Width = 126, Height = 28, Top = y };
            var refreshBtn = new ThemedButton { Text = scanning ? "스캔 중…" : "새로 고침", Icon = Icons.Refresh, Width = 108, Height = 28, Top = y, Enabled = !scanning };
            var showAll = new ThemedToggle { Checked = _showAllDiag, Top = y + 3 };
            var showAllLbl = new Label { Text = "모두 보기", AutoSize = true, ForeColor = Theme.Ink, Font = UiFont.Get(9f), Top = y + 6 };
            const int gap = 10;
            logBtn.Left = 24 + W - logBtn.Width;
            refreshBtn.Left = logBtn.Left - gap - refreshBtn.Width;
            showAll.Left = refreshBtn.Left - gap - showAll.Width;
            showAllLbl.Left = showAll.Left - 6 - TextRenderer.MeasureText(showAllLbl.Text, showAllLbl.Font).Width;
            showAll.CheckedChanged += (s, e) => { _showAllDiag = showAll.Checked; ShowPage(2); };
            refreshBtn.Click += (s, e) => _tray.Rescan();
            logBtn.Click += (s, e) => OpenLogFolder();
            page.Controls.Add(showAllLbl); page.Controls.Add(showAll); page.Controls.Add(refreshBtn); page.Controls.Add(logBtn);
            y += 58;

            var all = _tray.LastResults;
            var shown = new List<ScanResult>();
            foreach (var r in all) if (_showAllDiag || r.ChangeHostSupported) shown.Add(r);
            int switchableCount = _tray.Switchable().Count;

            var secLbl = SectionLabel("스캔된 기기", "전환 가능 " + switchableCount + "개"); secLbl.Location = new Point(24, y); page.Controls.Add(secLbl);
            y += 24;

            // header
            var head = BuildDiagHeader(W); head.Location = new Point(24, y); page.Controls.Add(head); y += 28;
            if (shown.Count == 0)
            {
                var none = new Label { Text = "표시할 기기가 없습니다. [새로 고침] 하세요.", AutoSize = true, ForeColor = Theme.Muted, Location = new Point(24, y) };
                page.Controls.Add(none); y += 30;
            }
            foreach (var r in shown)
            {
                var row = BuildDiagRow(r, W); row.Location = new Point(24, y); page.Controls.Add(row); y += 40;
            }
            y += 6;

            var noteIcon = new IconView(Icons.Plug, 14) { Left = 24, Top = y + 1, Tint = Theme.Muted };
            var note = new Label { Text = "기기 정보는 저장되어 매번 재검색하지 않음 · \"모두 보기\"를 켜면 빈 슬롯·리시버도 함께 표시", AutoSize = true, ForeColor = Theme.Muted, Font = UiFont.Get(8.5f), Left = 44, Top = y };
            page.Controls.Add(noteIcon); page.Controls.Add(note);
            y += 40;

            page.Height = y;
            return page;
        }

        private bool _showAllDiag;

        private Panel BuildDiagHeader(int w)
        {
            var p = new Panel { Width = w, Height = 26, BackColor = Theme.Content };
            AddCol(p, "기기", 0, (int)(w * 0.30));
            AddCol(p, "VID:PID", (int)(w * 0.30), (int)(w * 0.18));
            AddCol(p, "연결", (int)(w * 0.48), (int)(w * 0.22));
            AddCol(p, "0x1814 인덱스", (int)(w * 0.70), (int)(w * 0.18));
            AddCol(p, "채널", (int)(w * 0.88), (int)(w * 0.12));
            var sep = new Panel { Left = 0, Top = 25, Width = w, Height = 1, BackColor = Theme.Divider };
            p.Controls.Add(sep);
            return p;
        }

        private void AddCol(Panel p, string text, int x, int wcol)
        {
            p.Controls.Add(new Label { Text = text, AutoSize = false, Width = wcol, Height = 18, Left = x, Top = 4, Font = UiFont.Get(8f, FontStyle.Bold), ForeColor = Theme.Muted });
        }

        private Panel BuildDiagRow(ScanResult r, int w)
        {
            var p = new Panel { Width = w, Height = 40, BackColor = Theme.Content };
            bool isKeyboard = r.DeviceName != null && (r.DeviceName.IndexOf("Mechanical", StringComparison.OrdinalIgnoreCase) >= 0 || r.DeviceName.IndexOf("Keys", StringComparison.OrdinalIgnoreCase) >= 0 || r.DeviceName.IndexOf("keyboard", StringComparison.OrdinalIgnoreCase) >= 0);
            var icon = new IconView(isKeyboard ? (IconPaint)Icons.Keyboard : Icons.Mouse, 17) { Left = 0, Top = 11, Tint = Theme.Muted };
            var name = new Label { Text = r.DeviceName, AutoSize = false, Width = (int)(w * 0.30) - 26, Height = 18, Left = 26, Top = 11, Font = UiFont.Get(9f), ForeColor = Theme.Ink };
            p.Controls.Add(icon); p.Controls.Add(name);
            p.Controls.Add(ColLabel(r.VidPid, (int)(w * 0.30), (int)(w * 0.18), Theme.Muted));
            p.Controls.Add(ColLabel(r.ConnectionType, (int)(w * 0.48), (int)(w * 0.22), Theme.Ink));
            p.Controls.Add(ColLabel(r.ChangeHostSupported ? string.Format("0x{0:X2}", r.ChangeHostIndex) : "미지원", (int)(w * 0.70), (int)(w * 0.18), Theme.Ink));
            var badge = new ThemedBadge { Text = r.CurrentHost >= 0 ? "채널 " + (r.CurrentHost + 1) : "-", Left = (int)(w * 0.88), Top = 8, Height = 22, AutoWidth = true };
            p.Controls.Add(badge);
            var sep = new Panel { Left = 0, Top = 39, Width = w, Height = 1, BackColor = Theme.Divider };
            p.Controls.Add(sep);
            return p;
        }

        private Label ColLabel(string text, int x, int wcol, Color c)
        {
            return new Label { Text = text, AutoSize = false, Width = wcol, Height = 18, Left = x, Top = 11, Font = UiFont.Get(9f), ForeColor = c };
        }

        // ---- 전환 동작 ----
        private void DoSwitchOne(ScanResult device, int targetHost)
        {
            if (_busy) return;
            if (MessageBox.Show(this, string.Format("{0} 를 채널 {1} ({2}) 로 전환합니다.\r\n이 PC 에서 기기가 사라집니다. 진행할까요?",
                device.DeviceName, targetHost + 1, Config.AliasFor(targetHost)), "LogiFlow 전환",
                MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes) return;
            _busy = true;
            var t = new Thread(() =>
            {
                bool ok = HostSwitcher.SwitchOne(device, targetHost, Log);
                try { BeginInvoke((Action)(() => { _busy = false; MessageBox.Show(this, ok ? "전환 명령을 보냈습니다." : "전환 실패 — 로그를 확인하세요.", "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Information); })); }
                catch { _busy = false; }
            });
            t.IsBackground = true; t.Start();
        }

        private void OpenLogFolder()
        {
            try
            {
                Directory.CreateDirectory(Log.LogDirectory);
                Process.Start("explorer.exe", "\"" + Log.LogDirectory + "\"");
            }
            catch (Exception ex)
            {
                MessageBox.Show(this, "로그 폴더를 열지 못했습니다:\r\n" + ex.Message, "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            }
        }

        public void ShowSettings()
        {
            if (!Visible) Show();
            if (WindowState == FormWindowState.Minimized) WindowState = FormWindowState.Normal;
            Activate(); BringToFront();
            ShowPage(_pageIndex);
        }

        public void OnStateUpdated()
        {
            if (IsDisposed) return;
            ShowPage(_pageIndex);
        }

        private void OnFormClosing(object sender, FormClosingEventArgs e)
        {
            if (e.CloseReason == CloseReason.UserClosing)
            {
                e.Cancel = true;
                Hide();
            }
        }

        protected override void WndProc(ref Message m)
        {
            const int WM_DEVICECHANGE = 0x0219;
            if (m.Msg == WM_DEVICECHANGE)
            {
                try { _tray.OnDeviceChanged(); } catch { }
            }
            base.WndProc(ref m);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                Theme.Changed -= OnThemeChanged;
                UiFont.Changed -= OnFontChanged;
            }
            base.Dispose(disposing);
        }
    }
}
